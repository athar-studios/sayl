//! Core download engine tying together storage, HTTP transport, and domain state machines.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use chrono::Utc;
use sayl_core::error::CoreError;
use sayl_core::event::{EventKind, EventPayload, EventReceiver, EventSender};
use sayl_core::id::TaskId;
use sayl_core::state::DownloadState;
use sayl_http::probe::ProbeOptions;
use sayl_http::resume::{ResumeDecision, ResumeInfo};
use sayl_storage::{Checkpoint, Store, TaskRecord};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::config::EngineConfig;
use crate::lease::DEFAULT_LEASE_TTL;
use crate::queue::{Scheduler, TaskStatus as QueueTaskStatus};
use crate::sequential::DownloadOutcome;

struct ActiveTaskControl {
    cancel_token: CancellationToken,
    pause_token: CancellationToken,
}

struct TaskGuard {
    task_id: TaskId,
    active_tasks: Arc<Mutex<HashMap<TaskId, ActiveTaskControl>>>,
    _permit: OwnedSemaphorePermit,
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active_tasks.lock() {
            active.remove(&self.task_id);
        }
    }
}

struct EngineInner {
    download_dir: PathBuf,
    store: Arc<Store>,
    client: reqwest::Client,
    config: EngineConfig,
    event_sender: EventSender,
    event_receiver: Mutex<Option<EventReceiver>>,
    active_tasks: Arc<Mutex<HashMap<TaskId, ActiveTaskControl>>>,
    task_semaphore: Arc<Semaphore>,
    scheduler: Mutex<Scheduler>,
    /// Maps engine TaskIds to scheduler u64 task IDs.
    scheduler_id_map: Mutex<HashMap<TaskId, u64>>,
    /// Counter for generating unique scheduler task IDs.
    next_scheduler_id: Mutex<u64>,
}

/// Download engine orchestrating multi-task, sequential and parallel downloads.
#[derive(Clone)]
pub struct Engine {
    inner: Arc<EngineInner>,
}

impl Engine {
    /// Creates a new persistent download engine.
    pub fn new(
        download_dir: impl Into<PathBuf>,
        store_dir: impl Into<PathBuf>,
        config: EngineConfig,
    ) -> Result<Self, CoreError> {
        let dl_dir = download_dir.into();
        let st_dir = store_dir.into();
        std::fs::create_dir_all(&dl_dir)
            .map_err(|e| CoreError::io_with_context(e, "create download_dir"))?;
        std::fs::create_dir_all(&st_dir)
            .map_err(|e| CoreError::io_with_context(e, "create store_dir"))?;

        let store = Arc::new(Store::open(&st_dir)?);
        Self::with_store(dl_dir, store, config)
    }

    /// Creates an engine backed by an in-memory ephemeral store for testing.
    pub fn open_in_memory(
        download_dir: impl Into<PathBuf>,
        config: EngineConfig,
    ) -> Result<Self, CoreError> {
        let dl_dir = download_dir.into();
        std::fs::create_dir_all(&dl_dir)
            .map_err(|e| CoreError::io_with_context(e, "create download_dir"))?;
        let store = Arc::new(Store::open_in_memory()?);
        Self::with_store(dl_dir, store, config)
    }

    /// Creates an engine wrapping an existing storage instance.
    pub fn with_store(
        download_dir: impl Into<PathBuf>,
        store: Arc<Store>,
        config: EngineConfig,
    ) -> Result<Self, CoreError> {
        let dl_dir = download_dir.into();
        let (sender, receiver) = sayl_core::event::event_channel(10000);
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| CoreError::http(None, e.to_string(), None))?;

        let max_tasks = config.max_concurrent_tasks.max(1);
        let semaphore = Arc::new(Semaphore::new(max_tasks));
        let scheduler = Mutex::new(Scheduler::new(config.queue_limits));
        let scheduler_id_map = Mutex::new(HashMap::new());
        let next_scheduler_id = Mutex::new(1u64);

        let inner = EngineInner {
            download_dir: dl_dir,
            store,
            client,
            config,
            event_sender: sender,
            event_receiver: Mutex::new(Some(receiver)),
            active_tasks: Arc::new(Mutex::new(HashMap::new())),
            task_semaphore: semaphore,
            scheduler,
            scheduler_id_map,
            next_scheduler_id,
        };

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Probes the specified URL, derives a safe filename, persists a TaskRecord in Queued state,
    /// emits an event, and returns the task ID.
    pub async fn add(&self, url: impl AsRef<str>) -> Result<TaskId, CoreError> {
        let url_str = url.as_ref();
        let parsed_url =
            Url::parse(url_str).map_err(|e| CoreError::validation("url", e.to_string()))?;

        // Probe the URL
        let probe_res = sayl_http::probe::probe_with_options(
            &self.inner.client,
            parsed_url.clone(),
            ProbeOptions::default(),
        )
        .await
        .map_err(CoreError::from)?;

        let filename = probe_res.filename.clone().unwrap_or_else(|| {
            sayl_http::filename::derive_filename(None, &probe_res.final_url, None)
        });

        let task_id = TaskId::new();
        let mut record = TaskRecord::with_id(task_id, url_str, &self.inner.download_dir, &filename);
        record.final_url = Some(probe_res.final_url.to_string());
        record.total_size = probe_res.content_length;
        record.etag = probe_res.etag.clone();
        record.last_modified = probe_res.last_modified.clone();
        record.content_type = probe_res.content_type.clone();
        record.state = DownloadState::Queued;

        self.inner.store.upsert_task(&record)?;

        let _ = self
            .inner
            .event_sender
            .emit(
                task_id,
                EventKind::StateChanged,
                EventPayload::StateChanged {
                    from: DownloadState::Queued,
                    to: DownloadState::Queued,
                },
            )
            .await;

        Ok(task_id)
    }

    /// Spawns a background task executing `start` and returns its `JoinHandle`.
    pub fn spawn_start(&self, task_id: TaskId) -> tokio::task::JoinHandle<Result<(), CoreError>> {
        let engine = self.clone();
        tokio::spawn(async move { engine.start(task_id).await })
    }

    /// Runs the full lifecycle of a download task honoring `sayl_core::transition`:
    /// `Queued → Probing → Downloading → Verifying → Finalizing → Completed`,
    /// with `Pausing/Paused`, `Cancelling/Cancelled`, and `Failed` on error.
    ///
    /// This method now integrates with the scheduler: the task is enqueued and only
    /// starts downloading when the scheduler promotes it (global slot + origin permit available).
    pub async fn start(&self, task_id: TaskId) -> Result<(), CoreError> {
        // 1. Load task record
        let record =
            self.inner.store.get_task(task_id)?.ok_or_else(|| {
                CoreError::storage("not_found", format!("task {task_id} not found"))
            })?;

        // If already terminal, reject
        if record.state.is_terminal() {
            return Err(CoreError::validation(
                "state",
                format!("task {task_id} is in terminal state {}", record.state),
            ));
        }

        // Get or allocate scheduler ID for this task
        let sched_id = self.get_or_alloc_scheduler_id(task_id);

        // Enqueue in scheduler
        let origin = Self::extract_origin(&record.url)?;
        let promoted = {
            let mut scheduler = self
                .inner
                .scheduler
                .lock()
                .map_err(|_| CoreError::scheduler("scheduler mutex poisoned"))?;
            scheduler
                .enqueue(sched_id)
                .map_err(Self::queue_error_to_core)?;
            scheduler
                .set_origin(sched_id, &origin)
                .map_err(Self::queue_error_to_core)?;
            let started = scheduler.poll(Instant::now());
            started.contains(&sched_id)
        };

        if !promoted {
            // Not promoted: task stays queued, persist and return
            // The task is already in Queued state from add(), no state transition needed
            // Emit no Started event; the queued event from add() is sufficient
            return Ok(());
        }

        // Task was promoted: proceed with download
        self.run_download(task_id, record, sched_id).await
    }

    /// Converts a scheduler QueueError to CoreError.
    fn queue_error_to_core(err: crate::queue::QueueError) -> CoreError {
        CoreError::scheduler(err.to_string())
    }

    /// Gets the scheduler ID for a task, allocating a new one if needed.
    fn get_or_alloc_scheduler_id(&self, task_id: TaskId) -> u64 {
        {
            let map = self
                .inner
                .scheduler_id_map
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(&id) = map.get(&task_id) {
                return id;
            }
        }
        // Allocate new ID
        let mut next_id = self
            .inner
            .next_scheduler_id
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let new_id = *next_id;
        *next_id += 1;
        drop(next_id);
        let mut map = self
            .inner
            .scheduler_id_map
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        map.insert(task_id, new_id);
        new_id
    }

    /// Removes the scheduler ID mapping for a task.
    fn remove_scheduler_id(&self, task_id: TaskId) {
        let mut map = self
            .inner
            .scheduler_id_map
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        map.remove(&task_id);
    }

    /// Gets the scheduler ID for a task, returning None if not found.
    fn get_scheduler_id(&self, task_id: TaskId) -> Option<u64> {
        let map = self
            .inner
            .scheduler_id_map
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        map.get(&task_id).copied()
    }

    /// Internal method that executes the actual download after scheduler promotion.
    async fn run_download(
        &self,
        task_id: TaskId,
        mut record: TaskRecord,
        sched_id: u64,
    ) -> Result<(), CoreError> {
        // Verify state can transition to Probing
        sayl_core::transition(record.state, DownloadState::Probing)?;

        // Register active task control (no semaphore needed - scheduler manages concurrency)
        let cancel_token = CancellationToken::new();
        let pause_token = CancellationToken::new();
        {
            let mut active = self
                .inner
                .active_tasks
                .lock()
                .map_err(|_| CoreError::scheduler("active_tasks mutex poisoned"))?;
            if active.contains_key(&task_id) {
                return Err(CoreError::scheduler("task is already running"));
            }
            active.insert(
                task_id,
                ActiveTaskControl {
                    cancel_token: cancel_token.clone(),
                    pause_token: pause_token.clone(),
                },
            );
        }

        let _guard = TaskGuard {
            task_id,
            active_tasks: Arc::clone(&self.inner.active_tasks),
            _permit: self
                .inner
                .task_semaphore
                .clone()
                .try_acquire_owned()
                .map_err(|_| {
                    CoreError::scheduler(
                        "internal error: semaphore unavailable after scheduler promotion",
                    )
                })?,
        };

        // Transition to Probing
        self.transition_and_persist(&mut record, DownloadState::Probing)
            .await?;

        if cancel_token.is_cancelled() {
            self.handle_cancel_during_probing(task_id, &mut record)
                .await;
            let promoted = self.finish_and_get_promoted(sched_id, QueueTaskStatus::Cancelled);
            self.spawn_promoted_tasks(promoted);
            self.remove_scheduler_id(task_id);
            return Ok(());
        }
        if pause_token.is_cancelled() {
            self.handle_pause_during_probing(task_id, &mut record).await;
            self.scheduler_pause_and_promote(sched_id).await;
            return Ok(());
        }

        // Probe remote origin
        let target_url = Url::parse(record.final_url.as_deref().unwrap_or(&record.url))
            .map_err(|e| CoreError::validation("url", e.to_string()))?;

        let probe_res = match sayl_http::probe::probe_with_options(
            &self.inner.client,
            target_url,
            ProbeOptions::default(),
        )
        .await
        {
            Ok(res) => res,
            Err(err) => {
                let _ = self
                    .transition_and_persist(&mut record, DownloadState::Failed)
                    .await;
                let core_err = CoreError::from(err);
                record.error = Some(core_err.to_string());
                record.error_kind = Some(core_err.error_kind().to_string());
                let _ = self.inner.store.upsert_task(&record);
                let _ = self
                    .inner
                    .event_sender
                    .emit(
                        task_id,
                        EventKind::Error,
                        EventPayload::Error {
                            error: core_err.to_string(),
                            retryable: core_err.is_retryable(),
                        },
                    )
                    .await;
                let promoted = self.finish_and_get_promoted(sched_id, QueueTaskStatus::Failed);
                self.spawn_promoted_tasks(promoted);
                self.remove_scheduler_id(task_id);
                return Err(core_err);
            }
        };

        // Update record headers
        record.final_url = Some(probe_res.final_url.to_string());
        if probe_res.content_length.is_some() {
            record.total_size = probe_res.content_length;
        }
        record.etag = probe_res.etag.clone().or(record.etag.clone());
        record.last_modified = probe_res
            .last_modified
            .clone()
            .or(record.last_modified.clone());
        if probe_res.content_type.is_some() {
            record.content_type = probe_res.content_type.clone();
        }
        self.inner.store.upsert_task(&record)?;

        // Load checkpoint for resume support
        let checkpoint = self
            .inner
            .store
            .load_checkpoint(task_id)
            .map_err(CoreError::from)?;
        let (checkpoint_written, checkpoint_ranges) = if let Some(cp) = checkpoint {
            // Validate checkpoint against current task and probe
            let cp_valid = cp.total_size == record.total_size
                && cp.written <= record.total_size.unwrap_or(u64::MAX)
                && cp.range_map.iter().all(|(s, e)| s < e);
            if cp_valid {
                (cp.written, cp.range_map.clone())
            } else {
                (0u64, Vec::new())
            }
        } else {
            (0u64, Vec::new())
        };

        // Resume check
        let safe_filename = sayl_http::filename::sanitize_filename(&record.filename);
        if record.filename != safe_filename {
            record.filename = safe_filename.clone();
            let _ = self.inner.store.upsert_task(&record);
        }
        let part_filename = format!("{safe_filename}.part");
        let part_path = self.resolve_destination_path(&part_filename)?;
        let disk_len = if part_path.exists() {
            tokio::fs::metadata(&part_path)
                .await
                .map(|m| m.len())
                .unwrap_or(0)
        } else {
            0
        };

        // Use checkpoint.written for resume info if available and valid, else disk_len
        let resume_written = if checkpoint_written > 0 {
            checkpoint_written
        } else {
            disk_len
        };

        let mut resume_from = 0u64;
        let mut already_complete = false;
        let mut resume_info_opt = None;

        if resume_written > 0 || disk_len > 0 {
            let info = ResumeInfo {
                url: record.url.clone(),
                etag: record.etag.clone(),
                last_modified: record.last_modified.clone(),
                total_size: record.total_size,
                written: resume_written,
            };

            let val_decision = sayl_http::resume::validate_resume(
                &info,
                probe_res.etag.as_deref(),
                probe_res.last_modified.as_deref(),
                probe_res.content_length,
            );

            // Reconcile checkpoint/file length with actual disk length
            let decision = if disk_len < checkpoint_written && checkpoint_written > 0 {
                ResumeDecision::RestartFromZero
            } else if checkpoint_written == 0 && disk_len > 0 && record.total_size == Some(disk_len)
            {
                // Preallocated sparse file without durable chunk completion: restart from zero
                ResumeDecision::RestartFromZero
            } else {
                match val_decision {
                    ResumeDecision::ResumeFrom(_) => {
                        sayl_http::resume::reconcile_partial_len(&info, disk_len)
                    }
                    other => other,
                }
            };

            match decision {
                ResumeDecision::ResumeFrom(offset) => {
                    resume_from = offset;
                    if disk_len > offset {
                        let f = std::fs::OpenOptions::new()
                            .write(true)
                            .open(&part_path)
                            .map_err(|e| {
                                CoreError::io_with_context(e, "open part file for truncation")
                            })?;
                        f.set_len(offset).map_err(|e| {
                            CoreError::io_with_context(e, "truncate part file to resume offset")
                        })?;
                    }
                    resume_info_opt = Some(info);
                }
                ResumeDecision::RestartFromZero => {
                    resume_from = 0;
                    let f = std::fs::OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .create(true)
                        .open(&part_path)
                        .map_err(|e| CoreError::io_with_context(e, "truncate part file to zero"))?;
                    drop(f);
                    record.written = 0;
                    self.inner.store.upsert_task(&record)?;
                }
                ResumeDecision::AlreadyComplete => {
                    already_complete = true;
                }
                ResumeDecision::Reject(reason) => {
                    let _ = self
                        .transition_and_persist(&mut record, DownloadState::Failed)
                        .await;
                    record.error = Some(reason.to_string());
                    record.error_kind = Some("network".to_string());
                    let _ = self.inner.store.upsert_task(&record);
                    let promoted = self.finish_and_get_promoted(sched_id, QueueTaskStatus::Failed);
                    self.spawn_promoted_tasks(promoted);
                    self.remove_scheduler_id(task_id);
                    return Err(CoreError::validation("resume", reason));
                }
            }
        }

        // Final safety check: if file is shorter than resume_from, force restart from zero
        if resume_from > 0 && disk_len < resume_from {
            resume_from = 0;
            resume_info_opt = None;
            let f = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .create(true)
                .open(&part_path)
                .map_err(|e| CoreError::io_with_context(e, "truncate part file to zero"))?;
            drop(f);
            record.written = 0;
            self.inner.store.upsert_task(&record)?;
        }

        // Download execution
        let mut outcome_status = QueueTaskStatus::Finished;
        if !already_complete {
            self.transition_and_persist(&mut record, DownloadState::Downloading)
                .await?;

            let use_parallel = self.inner.config.max_connections > 1
                && probe_res.range_support.is_supported()
                && record.total_size.is_some()
                && record.total_size.unwrap() > 0
                && resume_from == 0;

            if use_parallel {
                let download_result = crate::parallel::run_parallel_download_with_options(
                    &self.inner.client,
                    &probe_res.final_url,
                    &part_path,
                    task_id,
                    record.total_size.unwrap(),
                    self.inner.config.max_connections,
                    self.inner.config.chunk_size,
                    cancel_token.clone(),
                    pause_token.clone(),
                    &self.inner.event_sender,
                    self.inner.store.clone(),
                    &checkpoint_ranges, // Pass completed ranges for resume
                    DEFAULT_LEASE_TTL,
                )
                .await;

                outcome_status = match download_result {
                    Ok((DownloadOutcome::Completed { written }, _report)) => {
                        record.written = written;
                        self.inner.store.upsert_task(&record)?;
                        QueueTaskStatus::Finished
                    }
                    Ok((DownloadOutcome::Paused { written }, _report)) => {
                        record.written = written;
                        let checkpoint = Checkpoint::new(
                            task_id,
                            written,
                            record.total_size,
                            checkpoint_ranges.clone(),
                        );
                        let _ = self.inner.store.save_checkpoint(&checkpoint);
                        self.transition_and_persist(&mut record, DownloadState::Pausing)
                            .await?;
                        self.transition_and_persist(&mut record, DownloadState::Paused)
                            .await?;
                        self.scheduler_pause_and_promote(sched_id).await;
                        return Ok(());
                    }
                    Ok((DownloadOutcome::Cancelled { written }, _report)) => {
                        record.written = written;
                        self.transition_and_persist(&mut record, DownloadState::Cancelling)
                            .await?;
                        self.transition_and_persist(&mut record, DownloadState::Cancelled)
                            .await?;
                        let _ = self
                            .inner
                            .event_sender
                            .emit(
                                task_id,
                                EventKind::Cancelled,
                                EventPayload::Cancelled {
                                    reason: "cancelled by user".into(),
                                },
                            )
                            .await;
                        QueueTaskStatus::Cancelled
                    }
                    Err(err) => {
                        // Fallback: if the server ignored Range requests (200 instead of 206),
                        // retry with sequential single-stream download instead of failing.
                        if err.error_kind() == "range_unsupported" {
                            tracing::warn!(
                                "parallel download unsupported (server ignores Range), falling back to sequential"
                            );
                            let seq_result = crate::sequential::run_sequential_download(
                                &self.inner.client,
                                &probe_res.final_url,
                                &part_path,
                                task_id,
                                record.total_size,
                                None,
                                0,
                                cancel_token.clone(),
                                pause_token.clone(),
                                &self.inner.event_sender,
                                &self.inner.store,
                            )
                            .await;
                            match seq_result {
                                Ok(DownloadOutcome::Completed { written }) => {
                                    record.written = written;
                                    self.inner.store.upsert_task(&record)?;
                                    QueueTaskStatus::Finished
                                }
                                Ok(DownloadOutcome::Paused { written }) => {
                                    record.written = written;
                                    let checkpoint = Checkpoint::new(
                                        task_id,
                                        written,
                                        record.total_size,
                                        vec![(0, written)],
                                    );
                                    let _ = self.inner.store.save_checkpoint(&checkpoint);
                                    self.transition_and_persist(
                                        &mut record,
                                        DownloadState::Pausing,
                                    )
                                    .await?;
                                    self.transition_and_persist(&mut record, DownloadState::Paused)
                                        .await?;
                                    self.scheduler_pause_and_promote(sched_id).await;
                                    return Ok(());
                                }
                                Ok(DownloadOutcome::Cancelled { written }) => {
                                    record.written = written;
                                    self.transition_and_persist(
                                        &mut record,
                                        DownloadState::Cancelling,
                                    )
                                    .await?;
                                    self.transition_and_persist(
                                        &mut record,
                                        DownloadState::Cancelled,
                                    )
                                    .await?;
                                    let _ = self
                                        .inner
                                        .event_sender
                                        .emit(
                                            task_id,
                                            EventKind::Cancelled,
                                            EventPayload::Cancelled {
                                                reason: "cancelled by user".into(),
                                            },
                                        )
                                        .await;
                                    QueueTaskStatus::Cancelled
                                }
                                Err(seq_err) => {
                                    let _ = self
                                        .transition_and_persist(&mut record, DownloadState::Failed)
                                        .await;
                                    record.error = Some(seq_err.to_string());
                                    record.error_kind = Some(seq_err.error_kind().to_string());
                                    let _ = self.inner.store.upsert_task(&record);
                                    let _ = self
                                        .inner
                                        .event_sender
                                        .emit(
                                            task_id,
                                            EventKind::Error,
                                            EventPayload::Error {
                                                error: seq_err.to_string(),
                                                retryable: seq_err.is_retryable(),
                                            },
                                        )
                                        .await;
                                    let promoted = self
                                        .finish_and_get_promoted(sched_id, QueueTaskStatus::Failed);
                                    self.spawn_promoted_tasks(promoted);
                                    self.remove_scheduler_id(task_id);
                                    return Err(seq_err);
                                }
                            }
                        } else {
                            let _ = self
                                .transition_and_persist(&mut record, DownloadState::Failed)
                                .await;
                            record.error = Some(err.to_string());
                            record.error_kind = Some(err.error_kind().to_string());
                            let _ = self.inner.store.upsert_task(&record);
                            let _ = self
                                .inner
                                .event_sender
                                .emit(
                                    task_id,
                                    EventKind::Error,
                                    EventPayload::Error {
                                        error: err.to_string(),
                                        retryable: err.is_retryable(),
                                    },
                                )
                                .await;
                            let promoted =
                                self.finish_and_get_promoted(sched_id, QueueTaskStatus::Failed);
                            self.spawn_promoted_tasks(promoted);
                            self.remove_scheduler_id(task_id);
                            return Err(err);
                        }
                    }
                };
            } else {
                let download_result = crate::sequential::run_sequential_download(
                    &self.inner.client,
                    &probe_res.final_url,
                    &part_path,
                    task_id,
                    record.total_size,
                    resume_info_opt.as_ref(),
                    resume_from,
                    cancel_token.clone(),
                    pause_token.clone(),
                    &self.inner.event_sender,
                    &self.inner.store,
                )
                .await;

                outcome_status = match download_result {
                    Ok(DownloadOutcome::Completed { written }) => {
                        record.written = written;
                        self.inner.store.upsert_task(&record)?;
                        QueueTaskStatus::Finished
                    }
                    Ok(DownloadOutcome::Paused { written }) => {
                        record.written = written;
                        let checkpoint = Checkpoint::new(
                            task_id,
                            written,
                            record.total_size,
                            vec![(0, written)],
                        );
                        let _ = self.inner.store.save_checkpoint(&checkpoint);
                        self.transition_and_persist(&mut record, DownloadState::Pausing)
                            .await?;
                        self.transition_and_persist(&mut record, DownloadState::Paused)
                            .await?;
                        self.scheduler_pause_and_promote(sched_id).await;
                        return Ok(());
                    }
                    Ok(DownloadOutcome::Cancelled { written }) => {
                        record.written = written;
                        self.transition_and_persist(&mut record, DownloadState::Cancelling)
                            .await?;
                        self.transition_and_persist(&mut record, DownloadState::Cancelled)
                            .await?;
                        let _ = self
                            .inner
                            .event_sender
                            .emit(
                                task_id,
                                EventKind::Cancelled,
                                EventPayload::Cancelled {
                                    reason: "cancelled by user".into(),
                                },
                            )
                            .await;
                        QueueTaskStatus::Cancelled
                    }
                    Err(err) => {
                        let _ = self
                            .transition_and_persist(&mut record, DownloadState::Failed)
                            .await;
                        record.error = Some(err.to_string());
                        record.error_kind = Some(err.error_kind().to_string());
                        let _ = self.inner.store.upsert_task(&record);
                        let _ = self
                            .inner
                            .event_sender
                            .emit(
                                task_id,
                                EventKind::Error,
                                EventPayload::Error {
                                    error: err.to_string(),
                                    retryable: err.is_retryable(),
                                },
                            )
                            .await;
                        let promoted =
                            self.finish_and_get_promoted(sched_id, QueueTaskStatus::Failed);
                        self.spawn_promoted_tasks(promoted);
                        self.remove_scheduler_id(task_id);
                        return Err(err);
                    }
                };
            }
        }

        // If already_complete or download completed, continue to verification/finalization
        if matches!(outcome_status, QueueTaskStatus::Finished) || already_complete {
            // Verification and finalization for successful downloads
            let final_status = self
                .complete_download(task_id, &mut record, &part_path)
                .await
                .unwrap_or(QueueTaskStatus::Failed);
            let promoted = self.finish_and_get_promoted(sched_id, final_status);
            self.spawn_promoted_tasks(promoted);
            if final_status.is_terminal() {
                self.remove_scheduler_id(task_id);
            }
        } else {
            let promoted = self.finish_and_get_promoted(sched_id, outcome_status);
            self.spawn_promoted_tasks(promoted);
            if outcome_status.is_terminal() {
                self.remove_scheduler_id(task_id);
            }
        }

        Ok(())
    }

    /// Completes a download after successful transfer: verify, finalize, emit Completed event.
    async fn complete_download(
        &self,
        task_id: TaskId,
        record: &mut TaskRecord,
        part_path: &Path,
    ) -> Result<QueueTaskStatus, CoreError> {
        // Verifying
        self.transition_and_persist(record, DownloadState::Verifying)
            .await?;

        let actual_len = tokio::fs::metadata(part_path)
            .await
            .map_err(|e| CoreError::io_with_context(e, "get part file metadata"))?
            .len();

        if let Some(expected) = record.total_size {
            if actual_len != expected {
                let _ = self
                    .transition_and_persist(record, DownloadState::Failed)
                    .await;
                record.error = Some("file size mismatch".to_string());
                record.error_kind = Some("disk".to_string());
                let _ = self.inner.store.upsert_task(record);
                return Ok(QueueTaskStatus::Failed);
            }
        }

        // Finalizing
        self.transition_and_persist(record, DownloadState::Finalizing)
            .await?;

        let final_path = self.resolve_destination_path(&record.filename)?;
        if final_path.exists() {
            let _ = tokio::fs::remove_file(&final_path).await;
        }
        tokio::fs::rename(part_path, &final_path)
            .await
            .map_err(|e| CoreError::io_with_context(e, "rename part file to final file"))?;

        // Completed
        self.transition_and_persist(record, DownloadState::Completed)
            .await?;
        record.written = actual_len;
        self.inner.store.upsert_task(record)?;

        let duration_ms = (Utc::now() - record.created_at).num_milliseconds().max(0) as u64;

        let _ = self
            .inner
            .event_sender
            .emit(
                task_id,
                EventKind::Completed,
                EventPayload::Completed {
                    total_bytes: actual_len,
                    duration_ms,
                },
            )
            .await;

        Ok(QueueTaskStatus::Finished)
    }

    /// Handles cancellation during probing phase.
    async fn handle_cancel_during_probing(&self, task_id: TaskId, record: &mut TaskRecord) {
        let _ = self
            .transition_and_persist(record, DownloadState::Cancelling)
            .await;
        let _ = self
            .transition_and_persist(record, DownloadState::Cancelled)
            .await;
        let _ = self
            .inner
            .event_sender
            .emit(
                task_id,
                EventKind::Cancelled,
                EventPayload::Cancelled {
                    reason: "cancelled during probing".into(),
                },
            )
            .await;
    }

    /// Handles pause during probing phase.
    async fn handle_pause_during_probing(&self, task_id: TaskId, record: &mut TaskRecord) {
        let checkpoint = Checkpoint::new(
            task_id,
            record.written,
            record.total_size,
            vec![(0, record.written)],
        );
        let _ = self.inner.store.save_checkpoint(&checkpoint);
        let _ = self
            .transition_and_persist(record, DownloadState::Pausing)
            .await;
        let _ = self
            .transition_and_persist(record, DownloadState::Paused)
            .await;
    }

    /// Calls scheduler.finish with outcome, polls for promoted tasks, and returns their scheduler IDs.
    fn finish_and_get_promoted(&self, sched_id: u64, outcome: QueueTaskStatus) -> Vec<u64> {
        let mut scheduler = match self.inner.scheduler.lock() {
            Ok(s) => s,
            Err(e) => e.into_inner(),
        };
        let _ = scheduler.finish(sched_id, outcome);
        scheduler.poll(Instant::now())
    }

    /// Cancels task in scheduler, polls for promoted tasks, and returns their scheduler IDs.
    fn cancel_and_get_promoted(&self, sched_id: u64) -> Vec<u64> {
        let mut scheduler = match self.inner.scheduler.lock() {
            Ok(s) => s,
            Err(e) => e.into_inner(),
        };
        let _ = scheduler.cancel(sched_id);
        scheduler.poll(Instant::now())
    }

    /// Spawns download for promoted scheduler IDs.
    fn spawn_promoted_tasks(&self, promoted_sched_ids: Vec<u64>) {
        for promoted_sched_id in promoted_sched_ids {
            let engine = self.clone();
            tokio::spawn(async move {
                if let Some(promoted_task_id) =
                    engine.get_task_id_from_scheduler_id(promoted_sched_id)
                {
                    let _ = engine
                        .run_promoted_download(promoted_task_id, promoted_sched_id)
                        .await;
                } else {
                    // No mapping: task was cancelled/removed. Release the scheduler slot.
                    let _ = engine
                        .finish_and_get_promoted(promoted_sched_id, QueueTaskStatus::Cancelled);
                    // Note: finish_and_get_promoted will poll and spawn any newly promoted tasks
                }
            });
        }
    }

    /// Runs download for a task that was promoted by the scheduler (skips scheduler enqueue).
    async fn run_promoted_download(&self, task_id: TaskId, sched_id: u64) -> Result<(), CoreError> {
        let record =
            self.inner.store.get_task(task_id)?.ok_or_else(|| {
                CoreError::storage("not_found", format!("task {task_id} not found"))
            })?;

        // If task is already terminal (cancelled while queued), don't start download.
        // Release the scheduler slot and promote waiters.
        if record.state.is_terminal() || record.state == DownloadState::Cancelling {
            let promoted = self.finish_and_get_promoted(sched_id, QueueTaskStatus::Cancelled);
            self.spawn_promoted_tasks(promoted);
            return Ok(());
        }

        self.run_download(task_id, record, sched_id).await
    }

    /// Releases scheduler slot for paused task and promotes waiting tasks.
    async fn scheduler_pause_and_promote(&self, sched_id: u64) {
        let promoted = self.cancel_and_get_promoted(sched_id);
        self.spawn_promoted_tasks(promoted);
        // Remove scheduler ID mapping for paused task so it can be re-enqueued on resume
        if let Some(task_id) = self.get_task_id_from_scheduler_id(sched_id) {
            self.remove_scheduler_id(task_id);
        }
    }

    /// Gets the TaskId for a scheduler ID, if mapped.
    fn get_task_id_from_scheduler_id(&self, sched_id: u64) -> Option<TaskId> {
        let map = self
            .inner
            .scheduler_id_map
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // Reverse lookup: find TaskId by scheduler ID (value)
        map.iter()
            .find_map(|(&task_id, &v)| if v == sched_id { Some(task_id) } else { None })
    }

    /// Extracts the origin (scheme://host) from a URL string.
    fn extract_origin(url: &str) -> Result<String, CoreError> {
        let parsed = Url::parse(url).map_err(|e| CoreError::validation("url", e.to_string()))?;
        let scheme = parsed.scheme();
        let host = parsed
            .host_str()
            .ok_or_else(|| CoreError::validation("url", "missing host"))?;
        let port = parsed.port().map(|p| format!(":{p}")).unwrap_or_default();
        Ok(format!("{scheme}://{host}{port}"))
    }

    /// Pauses an active or queued download task.
    pub async fn pause(&self, task_id: TaskId) -> Result<(), CoreError> {
        if let Ok(active) = self.inner.active_tasks.lock() {
            if let Some(ctrl) = active.get(&task_id) {
                ctrl.pause_token.cancel();
            }
        }

        tokio::task::yield_now().await;

        let mut record = match self.inner.store.get_task(task_id)? {
            Some(r) => r,
            None => {
                return Err(CoreError::storage(
                    "not_found",
                    format!("task {task_id} not found"),
                ))
            }
        };

        if record.state == DownloadState::Paused {
            return Ok(());
        }

        let is_active = self
            .inner
            .active_tasks
            .lock()
            .map(|a| a.contains_key(&task_id))
            .unwrap_or(false);

        if !is_active && record.state == DownloadState::Queued {
            // Task is queued but not running: remove from scheduler and mark as Paused
            if let Some(sched_id) = self.get_scheduler_id(task_id) {
                self.scheduler_pause_queued(sched_id);
            }
            if self
                .transition_and_persist(&mut record, DownloadState::Probing)
                .await
                .is_ok()
            {
                let _ = self
                    .transition_and_persist(&mut record, DownloadState::Pausing)
                    .await;
                let _ = self
                    .transition_and_persist(&mut record, DownloadState::Paused)
                    .await;
            }
        }

        Ok(())
    }

    /// Removes a queued task from scheduler and promotes waiting tasks.
    fn scheduler_pause_queued(&self, sched_id: u64) {
        let promoted = self.cancel_and_get_promoted(sched_id);
        self.spawn_promoted_tasks(promoted);
    }

    /// Cancels a download task. Idempotent: repeated calls succeed without error.
    pub async fn cancel(&self, task_id: TaskId) -> Result<(), CoreError> {
        // Signal cancellation to active download if running
        if let Ok(active) = self.inner.active_tasks.lock() {
            if let Some(ctrl) = active.get(&task_id) {
                ctrl.cancel_token.cancel();
            }
        }

        tokio::task::yield_now().await;

        let mut record = match self.inner.store.get_task(task_id)? {
            Some(r) => r,
            None => {
                return Err(CoreError::storage(
                    "not_found",
                    format!("task {task_id} not found"),
                ))
            }
        };

        if record.state == DownloadState::Cancelled {
            // Still need to clean up scheduler
            self.scheduler_cancel_and_promote(task_id).await;
            return Ok(());
        }
        if record.state.is_terminal() {
            self.scheduler_cancel_and_promote(task_id).await;
            return Ok(());
        }

        let is_active = self
            .inner
            .active_tasks
            .lock()
            .map(|a| a.contains_key(&task_id))
            .unwrap_or(false);

        if !is_active {
            // Task is queued but not running: cancel directly in scheduler and persist
            if sayl_core::transition(record.state, DownloadState::Cancelling).is_ok() {
                self.transition_and_persist(&mut record, DownloadState::Cancelling)
                    .await?;
                self.transition_and_persist(&mut record, DownloadState::Cancelled)
                    .await?;
                let _ = self
                    .inner
                    .event_sender
                    .emit(
                        task_id,
                        EventKind::Cancelled,
                        EventPayload::Cancelled {
                            reason: "cancelled by user".into(),
                        },
                    )
                    .await;
            }
            // Cancel in scheduler FIRST, then remove mapping.
            // This ensures the queued task is actually removed from the scheduler.
            if let Some(sched_id) = self.get_scheduler_id(task_id) {
                let promoted = self.cancel_and_get_promoted(sched_id);
                self.spawn_promoted_tasks(promoted);
                self.remove_scheduler_id(task_id);
            }
        }
        // If active, the running download will handle the cancellation and call on_task_complete

        // Update scheduler and promote waiting tasks (for active tasks)
        self.scheduler_cancel_and_promote(task_id).await;

        Ok(())
    }

    /// Internal: cancel task in scheduler and promote waiting tasks.
    async fn scheduler_cancel_and_promote(&self, task_id: TaskId) {
        let sched_id = match self.get_scheduler_id(task_id) {
            Some(id) => id,
            None => return, // No scheduler entry, nothing to do
        };

        let promoted = self.cancel_and_get_promoted(sched_id);
        self.spawn_promoted_tasks(promoted);

        // Clean up scheduler ID mapping
        self.remove_scheduler_id(task_id);
    }

    /// Reloads tasks from the store and recovers non-terminal tasks to Paused/Queued or Failed.
    pub async fn recover(&self) -> Result<Vec<TaskId>, CoreError> {
        let tasks = self.inner.store.list_tasks().map_err(CoreError::from)?;
        let mut updated = Vec::new();

        for mut task in tasks {
            if task.state.is_terminal() {
                continue;
            }

            let part_path = self
                .inner
                .download_dir
                .join(format!("{}.part", task.filename));
            let part_len = if part_path.exists() {
                tokio::fs::metadata(&part_path)
                    .await
                    .map(|m| m.len())
                    .unwrap_or(0)
            } else {
                0
            };

            // Load checkpoint if exists
            let checkpoint = self
                .inner
                .store
                .load_checkpoint(task.id)
                .map_err(CoreError::from)?;
            let mut checkpoint_valid = false;
            let mut checkpoint_written = 0u64;
            let mut _checkpoint_ranges: Vec<(u64, u64)> = Vec::new();

            if let Some(cp) = checkpoint {
                // Validate checkpoint against task and file
                // Note: we allow part_len < cp.written (partial file shorter than checkpoint)
                // as this is handled separately below
                let cp_corrupt = cp.total_size.is_some()
                    && task.total_size.is_some()
                    && cp.total_size != task.total_size
                    || cp.written > task.total_size.unwrap_or(u64::MAX)
                    || cp.range_map.iter().any(|(s, e)| s >= e);

                if !cp_corrupt {
                    checkpoint_valid = true;
                    checkpoint_written = cp.written;
                    _checkpoint_ranges = cp.range_map.clone();
                    // Update task.written from checkpoint if it's more recent
                    if cp.written > task.written {
                        task.written = cp.written;
                        let _ = self.inner.store.upsert_task(&task);
                    }
                } else {
                    // Corrupt checkpoint: discard it
                    let _ = self.inner.store.save_checkpoint(&Checkpoint::new(
                        task.id,
                        0,
                        task.total_size,
                        Vec::new(),
                    ));
                }
            }

            // Check if final file exists and is complete (handles crash after write before finalize)
            let final_path = self.inner.download_dir.join(&task.filename);
            let final_exists = final_path.exists();
            let final_len = if final_exists {
                tokio::fs::metadata(&final_path)
                    .await
                    .map(|m| m.len())
                    .unwrap_or(0)
            } else {
                0
            };
            let final_complete =
                final_exists && task.total_size.is_some() && final_len >= task.total_size.unwrap();

            // Handle partial file shorter than checkpoint: discard checkpoint entirely to force full restart
            if checkpoint_valid && part_len < checkpoint_written && !final_complete {
                let new_checkpoint = Checkpoint::new(task.id, 0, task.total_size, Vec::new());
                self.inner
                    .store
                    .save_checkpoint(&new_checkpoint)
                    .map_err(CoreError::from)?;
                checkpoint_valid = false;
                checkpoint_written = 0;
                task.written = 0;
                self.inner.store.upsert_task(&task)?;
            }

            let is_corrupt = task.url.is_empty()
                || task.filename.is_empty()
                || (task.total_size.is_some() && part_len > task.total_size.unwrap())
                || (checkpoint_valid && part_len < checkpoint_written && !final_complete);

            if is_corrupt {
                let _ = self
                    .transition_and_persist(&mut task, DownloadState::Failed)
                    .await;
                updated.push(task.id);
                continue;
            }

            match task.state {
                DownloadState::Queued => {
                    // Re-enqueue in scheduler
                    let sched_id = self.get_or_alloc_scheduler_id(task.id);
                    let origin = Self::extract_origin(&task.url)?;
                    let mut scheduler = self
                        .inner
                        .scheduler
                        .lock()
                        .map_err(|_| CoreError::scheduler("scheduler mutex poisoned"))?;
                    scheduler
                        .enqueue(sched_id)
                        .map_err(Self::queue_error_to_core)?;
                    scheduler
                        .set_origin(sched_id, &origin)
                        .map_err(Self::queue_error_to_core)?;
                    // Note: we don't poll here; recovered tasks wait for their turn
                }
                DownloadState::Paused => {
                    // Dormant valid state, not enqueued in scheduler
                }
                DownloadState::Probing | DownloadState::Downloading => {
                    if self
                        .transition_and_persist(&mut task, DownloadState::Pausing)
                        .await
                        .is_ok()
                    {
                        let _ = self
                            .transition_and_persist(&mut task, DownloadState::Paused)
                            .await;
                        updated.push(task.id);
                    } else {
                        let _ = self
                            .transition_and_persist(&mut task, DownloadState::Failed)
                            .await;
                        updated.push(task.id);
                    }
                }
                DownloadState::Pausing => {
                    if self
                        .transition_and_persist(&mut task, DownloadState::Paused)
                        .await
                        .is_ok()
                    {
                        updated.push(task.id);
                    }
                }
                DownloadState::Cancelling => {
                    if self
                        .transition_and_persist(&mut task, DownloadState::Cancelled)
                        .await
                        .is_ok()
                    {
                        updated.push(task.id);
                    }
                }
                DownloadState::Verifying | DownloadState::Finalizing => {
                    // If file is complete, finalize; else fail
                    if task.total_size.is_some() && part_len >= task.total_size.unwrap() {
                        // File appears complete, transition to Finalizing then let start() handle it
                        if self
                            .transition_and_persist(&mut task, DownloadState::Finalizing)
                            .await
                            .is_ok()
                        {
                            updated.push(task.id);
                        }
                    } else {
                        if self
                            .transition_and_persist(&mut task, DownloadState::Failed)
                            .await
                            .is_ok()
                        {
                            updated.push(task.id);
                        }
                    }
                }
                _ => {}
            }
        }

        // After recovering all tasks, poll scheduler to promote any queued tasks
        let promoted = {
            let mut scheduler = self
                .inner
                .scheduler
                .lock()
                .map_err(|_| CoreError::scheduler("scheduler mutex poisoned"))?;
            scheduler.poll(Instant::now())
        };
        self.spawn_promoted_tasks(promoted);

        Ok(updated)
    }

    /// Fetches a task record by ID.
    pub fn get_task(&self, task_id: TaskId) -> Result<Option<TaskRecord>, CoreError> {
        self.inner.store.get_task(task_id).map_err(CoreError::from)
    }

    /// Returns a list of all stored task records.
    pub fn list_tasks(&self) -> Result<Vec<TaskRecord>, CoreError> {
        self.inner.store.list_tasks().map_err(CoreError::from)
    }

    /// Loads the checkpoint for a task.
    pub fn load_checkpoint(&self, task_id: TaskId) -> Result<Option<Checkpoint>, CoreError> {
        self.inner
            .store
            .load_checkpoint(task_id)
            .map_err(CoreError::from)
    }

    /// Deletes a task from the store.
    pub fn delete_task(&self, task_id: TaskId) -> Result<(), CoreError> {
        self.inner
            .store
            .delete_task(task_id)
            .map_err(CoreError::from)
    }

    /// Takes the receiver handle for listening to events emitted by the engine.
    pub fn take_event_receiver(&self) -> Option<EventReceiver> {
        self.inner.event_receiver.lock().ok()?.take()
    }

    /// Returns a reference to the event sender handle.
    pub fn event_sender(&self) -> &EventSender {
        &self.inner.event_sender
    }

    /// Returns a reference to the engine configuration.
    pub fn config(&self) -> &EngineConfig {
        &self.inner.config
    }

    /// Returns the download directory path.
    pub fn download_dir(&self) -> &Path {
        &self.inner.download_dir
    }

    /// Returns a snapshot of the scheduler occupancy.
    pub async fn queue_stats(&self) -> crate::queue::QueueStats {
        let scheduler = self
            .inner
            .scheduler
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        scheduler.stats()
    }

    /// Helper that validates and persists state transitions.
    async fn transition_and_persist(
        &self,
        record: &mut TaskRecord,
        target: DownloadState,
    ) -> Result<(), CoreError> {
        let old_state = record.state;
        let new_state = sayl_core::transition(old_state, target)?;
        record.state = new_state;
        record.updated_at = Utc::now();
        self.inner.store.upsert_task(record)?;
        let _ = self
            .inner
            .event_sender
            .emit(
                record.id,
                EventKind::StateChanged,
                EventPayload::StateChanged {
                    from: old_state,
                    to: new_state,
                },
            )
            .await;
        Ok(())
    }

    /// Resolves a safe destination path within `download_dir` from an untrusted filename,
    /// re-sanitizing it and asserting `download_dir` is its ancestor.
    pub fn resolve_destination_path(&self, raw_filename: &str) -> Result<PathBuf, CoreError> {
        let safe_filename = sayl_http::filename::sanitize_filename(raw_filename);
        let target_path = self.inner.download_dir.join(&safe_filename);
        if !target_path.starts_with(&self.inner.download_dir) {
            return Err(CoreError::scheduler(format!(
                "destination path traversal detected: '{raw_filename}' escapes download directory"
            )));
        }
        Ok(target_path)
    }

    #[cfg(test)]
    pub(crate) fn poison_active_tasks_for_test(&self) {
        let lock = std::sync::Arc::clone(&self.inner.active_tasks);
        let _ = std::thread::spawn(move || {
            let _guard = lock.lock().unwrap();
            panic!("deliberately poisoning active_tasks mutex for test");
        })
        .join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_temp_dir() -> tempfile::TempDir {
        if let Ok(cargo_tmp) = std::env::var("CARGO_TARGET_TMPDIR") {
            let _ = std::fs::create_dir_all(&cargo_tmp);
            if let Ok(dir) = tempfile::tempdir_in(&cargo_tmp) {
                return dir;
            }
        }
        let target_tmp = std::path::Path::new("target").join("test-tmp");
        let _ = std::fs::create_dir_all(&target_tmp);
        if let Ok(dir) = tempfile::tempdir_in(&target_tmp) {
            return dir;
        }
        tempfile::tempdir().expect("create temp dir")
    }

    #[tokio::test]
    async fn test_poisoned_mutex_handled_gracefully() {
        let temp_dir = make_test_temp_dir();
        let engine =
            Engine::open_in_memory(temp_dir.path(), EngineConfig::default()).expect("open engine");

        let task_id = TaskId::new();
        let record = TaskRecord::with_id(
            task_id,
            "http://example.com/test.zip",
            engine.download_dir(),
            "test.zip",
        );
        engine
            .inner
            .store
            .upsert_task(&record)
            .expect("upsert task");

        // Deliberately poison active_tasks mutex from a background thread
        engine.poison_active_tasks_for_test();

        // Starting the task must return a CoreError rather than panicking
        let result = engine.start(task_id).await;
        assert!(result.is_err(), "expected error on poisoned mutex, got Ok");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("poisoned"),
            "expected error message to mention poisoning, got: {err_msg}"
        );
    }

    #[test]
    fn test_resolve_destination_path_resanitizes_and_prevents_traversal() {
        let temp_dir = make_test_temp_dir();
        let engine =
            Engine::open_in_memory(temp_dir.path(), EngineConfig::default()).expect("open engine");

        // 1. Path traversal attempt: ..\..\evil.exe
        let path_traversal = engine
            .resolve_destination_path("..\\..\\evil.exe")
            .expect("resolves safely");
        assert!(path_traversal.starts_with(engine.download_dir()));
        assert_eq!(path_traversal, temp_dir.path().join("evil.exe"));

        // Also test unix style traversal
        let unix_traversal = engine
            .resolve_destination_path("../../evil.exe")
            .expect("resolves safely");
        assert!(unix_traversal.starts_with(engine.download_dir()));
        assert_eq!(unix_traversal, temp_dir.path().join("evil.exe"));

        // 2. Absolute path attempt
        let abs_path = engine
            .resolve_destination_path("C:\\Windows\\System32\\calc.exe")
            .expect("resolves safely");
        assert!(abs_path.starts_with(engine.download_dir()));
        assert_eq!(abs_path, temp_dir.path().join("calc.exe"));
    }
}

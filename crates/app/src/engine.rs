//! Core download engine tying together storage, HTTP transport, and domain state machines.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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
    store: Store,
    client: reqwest::Client,
    config: EngineConfig,
    event_sender: EventSender,
    event_receiver: Mutex<Option<EventReceiver>>,
    active_tasks: Arc<Mutex<HashMap<TaskId, ActiveTaskControl>>>,
    task_semaphore: Arc<Semaphore>,
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

        let store = Store::open(&st_dir)?;
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
        let store = Store::open_in_memory()?;
        Self::with_store(dl_dir, store, config)
    }

    /// Creates an engine wrapping an existing storage instance.
    pub fn with_store(
        download_dir: impl Into<PathBuf>,
        store: Store,
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

        let inner = EngineInner {
            download_dir: dl_dir,
            store,
            client,
            config,
            event_sender: sender,
            event_receiver: Mutex::new(Some(receiver)),
            active_tasks: Arc::new(Mutex::new(HashMap::new())),
            task_semaphore: semaphore,
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
        let mut record = TaskRecord::with_id(
            task_id,
            url_str,
            &self.inner.download_dir,
            &filename,
        );
        record.final_url = Some(probe_res.final_url.to_string());
        record.total_size = probe_res.content_length;
        record.etag = probe_res.etag.clone();
        record.last_modified = probe_res.last_modified.clone();
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
    pub async fn start(&self, task_id: TaskId) -> Result<(), CoreError> {
        // 1. Load task record
        let mut record = self
            .inner
            .store
            .get_task(task_id)?
            .ok_or_else(|| CoreError::storage("not_found", format!("task {task_id} not found")))?;

        // If already terminal, reject or return
        if record.state.is_terminal() {
            return Err(CoreError::validation(
                "state",
                format!("task {task_id} is in terminal state {}", record.state),
            ));
        }

        // Verify state can transition to Probing
        sayl_core::transition(record.state, DownloadState::Probing)?;

        // 2. Enforce concurrency limit: acquire semaphore permit
        let permit = self
            .inner
            .task_semaphore
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                CoreError::scheduler("concurrency limit reached: max concurrent tasks running")
            })?;

        // 3. Register active task control
        let cancel_token = CancellationToken::new();
        let pause_token = CancellationToken::new();
        {
            let mut active = self.inner.active_tasks.lock().unwrap();
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
            _permit: permit,
        };

        // 4. Transition to Probing
        self.transition_and_persist(&mut record, DownloadState::Probing)
            .await?;

        if cancel_token.is_cancelled() {
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
                        reason: "cancelled during probing".into(),
                    },
                )
                .await;
            return Ok(());
        }
        if pause_token.is_cancelled() {
            let checkpoint = Checkpoint::new(
                task_id,
                record.written,
                record.total_size,
                vec![(0, record.written)],
            );
            let _ = self.inner.store.save_checkpoint(&checkpoint);
            self.transition_and_persist(&mut record, DownloadState::Pausing)
                .await?;
            self.transition_and_persist(&mut record, DownloadState::Paused)
                .await?;
            return Ok(());
        }

        // 5. Probe remote origin
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
                record.error = Some(err.to_string());
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
                return Err(err.into());
            }
        };

        // Update record headers
        record.final_url = Some(probe_res.final_url.to_string());
        if probe_res.content_length.is_some() {
            record.total_size = probe_res.content_length;
        }
        record.etag = probe_res.etag.clone().or(record.etag.clone());
        record.last_modified = probe_res.last_modified.clone().or(record.last_modified.clone());
        self.inner.store.upsert_task(&record)?;

        // 6. Resume check
        let part_path = self
            .inner
            .download_dir
            .join(format!("{}.part", record.filename));
        let disk_len = if part_path.exists() {
            tokio::fs::metadata(&part_path)
                .await
                .map(|m| m.len())
                .unwrap_or(0)
        } else {
            0
        };

        let mut resume_from = 0u64;
        let mut already_complete = false;
        let mut resume_info_opt = None;

        if disk_len > 0 {
            let info = ResumeInfo {
                url: record.url.clone(),
                etag: record.etag.clone(),
                last_modified: record.last_modified.clone(),
                total_size: record.total_size,
                written: disk_len,
            };

            let val_decision = sayl_http::resume::validate_resume(
                &info,
                probe_res.etag.as_deref(),
                probe_res.last_modified.as_deref(),
                probe_res.content_length,
            );

            let decision = match val_decision {
                ResumeDecision::ResumeFrom(_) => {
                    sayl_http::resume::reconcile_partial_len(&info, disk_len)
                }
                other => other,
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
                        .map_err(|e| {
                            CoreError::io_with_context(e, "truncate part file to zero")
                        })?;
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
                    let _ = self.inner.store.upsert_task(&record);
                    return Err(CoreError::validation("resume", reason));
                }
            }
        }

        // 7. Download execution
        if !already_complete {
            self.transition_and_persist(&mut record, DownloadState::Downloading)
                .await?;

            let use_parallel = self.inner.config.max_connections > 1
                && probe_res.range_support.is_supported()
                && record.total_size.is_some()
                && record.total_size.unwrap() > 0
                && resume_from == 0;

            let download_result = if use_parallel {
                crate::parallel::run_parallel_download(
                    &self.inner.client,
                    &probe_res.final_url,
                    &part_path,
                    task_id,
                    record.total_size.unwrap(),
                    self.inner.config.max_connections,
                    cancel_token.clone(),
                    pause_token.clone(),
                    &self.inner.event_sender,
                    &self.inner.store,
                )
                .await
            } else {
                crate::sequential::run_sequential_download(
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
                .await
            };

            match download_result {
                Ok(DownloadOutcome::Completed { written }) => {
                    record.written = written;
                    self.inner.store.upsert_task(&record)?;
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
                    return Ok(());
                }
                Err(err) => {
                    let _ = self
                        .transition_and_persist(&mut record, DownloadState::Failed)
                        .await;
                    record.error = Some(err.to_string());
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
                    return Err(err);
                }
            }
        }

        // 8. Verifying
        self.transition_and_persist(&mut record, DownloadState::Verifying)
            .await?;

        let actual_len = tokio::fs::metadata(&part_path)
            .await
            .map_err(|e| CoreError::io_with_context(e, "get part file metadata"))?
            .len();

        if let Some(expected) = record.total_size {
            if actual_len != expected {
                let _ = self
                    .transition_and_persist(&mut record, DownloadState::Failed)
                    .await;
                record.error = Some("file size mismatch".to_string());
                let _ = self.inner.store.upsert_task(&record);
                return Err(CoreError::validation(
                    "content_length",
                    format!("verified size {actual_len} does not match expected {expected}"),
                ));
            }
        }

        // 9. Finalizing
        self.transition_and_persist(&mut record, DownloadState::Finalizing)
            .await?;

        let final_path = self.inner.download_dir.join(&record.filename);
        if final_path.exists() {
            let _ = tokio::fs::remove_file(&final_path).await;
        }
        tokio::fs::rename(&part_path, &final_path)
            .await
            .map_err(|e| CoreError::io_with_context(e, "rename part file to final file"))?;

        // 10. Completed
        self.transition_and_persist(&mut record, DownloadState::Completed)
            .await?;
        record.written = actual_len;
        self.inner.store.upsert_task(&record)?;

        let duration_ms = (Utc::now() - record.created_at)
            .num_milliseconds()
            .max(0) as u64;

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

        Ok(())
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

    /// Cancels a download task. Idempotent: repeated calls succeed without error.
    pub async fn cancel(&self, task_id: TaskId) -> Result<(), CoreError> {
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
            return Ok(());
        }
        if record.state.is_terminal() {
            return Ok(());
        }

        let is_active = self
            .inner
            .active_tasks
            .lock()
            .map(|a| a.contains_key(&task_id))
            .unwrap_or(false);

        if !is_active {
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
        }

        Ok(())
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

            let is_corrupt = task.url.is_empty()
                || task.filename.is_empty()
                || (task.total_size.is_some() && part_len > task.total_size.unwrap());

            if is_corrupt {
                let _ = self
                    .transition_and_persist(&mut task, DownloadState::Failed)
                    .await;
                updated.push(task.id);
                continue;
            }

            match task.state {
                DownloadState::Queued | DownloadState::Paused => {
                    // Dormant valid states
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
                    if self
                        .transition_and_persist(&mut task, DownloadState::Failed)
                        .await
                        .is_ok()
                    {
                        updated.push(task.id);
                    }
                }
                _ => {}
            }
        }

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
}

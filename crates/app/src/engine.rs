//! Core download engine tying together storage, HTTP transport, and domain state machines.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
use crate::parallel::SegmentRange;
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
    /// In-memory custom request headers per task (never persisted to journal).
    task_headers: Mutex<HashMap<TaskId, Vec<(String, String)>>>,
    /// Live per-task speed caps in bytes/sec (0 = unlimited), shared with the
    /// running transfer so a UI edit throttles without restarting the task.
    speed_limit_cells: Mutex<HashMap<TaskId, Arc<AtomicU64>>>,
    /// Duration of stall timeout without progress in milliseconds.
    stall_timeout_ms: AtomicU64,
}

/// Download engine orchestrating multi-task, sequential and parallel downloads.
#[derive(Clone)]
pub struct Engine {
    inner: Arc<EngineInner>,
}

/// Calculates the contiguous byte range starting from offset 0 across all ranges.
fn contiguous_prefix(ranges: &[(u64, u64)]) -> u64 {
    let mut sorted = ranges.to_vec();
    sorted.sort_by_key(|r| r.0);
    let mut offset = 0u64;
    for (start, end) in sorted {
        if start <= offset {
            if end > offset {
                offset = end;
            }
        } else {
            break;
        }
    }
    offset
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
        let stall_timeout_ms = AtomicU64::new(config.stall_timeout.as_millis() as u64);
        let task_headers = Mutex::new(HashMap::new());

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
            task_headers,
            speed_limit_cells: Mutex::new(HashMap::new()),
            stall_timeout_ms,
        };

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Returns the live speed-limit cell (bytes/sec) for a task, creating it on
    /// first use and seeding it with the task's persisted limit.
    fn speed_limit_cell(&self, task_id: TaskId, initial_kbps: Option<u32>) -> Arc<AtomicU64> {
        let cell = {
            let mut cells = match self.inner.speed_limit_cells.lock() {
                Ok(cells) => cells,
                Err(_) => return Arc::new(AtomicU64::new(0)),
            };
            Arc::clone(
                cells
                    .entry(task_id)
                    .or_insert_with(|| Arc::new(AtomicU64::new(0))),
            )
        };
        cell.store(initial_kbps.unwrap_or(0) as u64 * 1024, Ordering::Relaxed);
        cell
    }

    /// Sets or clears a task speed cap in KB/s. `None` and `0` mean unlimited.
    ///
    /// The value is persisted and, when the task is running, applied live to
    /// the transfer's token bucket.
    pub fn set_speed_limit(&self, task_id: TaskId, kbps: Option<u32>) -> Result<(), CoreError> {
        let normalized = kbps.filter(|kbps| *kbps > 0);
        if let Some(mut record) = self.inner.store.get_task(task_id)? {
            record.speed_limit_kbps = normalized;
            record.updated_at = Utc::now();
            self.inner.store.upsert_task(&record)?;
        }
        if let Ok(cells) = self.inner.speed_limit_cells.lock() {
            if let Some(cell) = cells.get(&task_id) {
                cell.store(normalized.unwrap_or(0) as u64 * 1024, Ordering::Relaxed);
            }
        }
        Ok(())
    }

    /// Probes the specified URL, derives a safe filename, persists a TaskRecord in Queued state,
    /// emits an event, and returns the task ID.
    pub async fn add(&self, url: impl AsRef<str>) -> Result<TaskId, CoreError> {
        let url_str = url.as_ref();
        let parsed_url =
            Url::parse(url_str).map_err(|e| CoreError::validation("url", e.to_string()))?;

        // Probe the URL (best-effort to populate metadata; full probing and error handling runs in run_download)
        let probe_res = sayl_http::probe::probe_with_options(
            &self.inner.client,
            parsed_url.clone(),
            ProbeOptions::default(),
        )
        .await;

        let (filename, final_url, total_size, etag, last_modified, content_type) = match probe_res {
            Ok(res) => {
                let derived = res.filename.clone().unwrap_or_else(|| {
                    sayl_http::filename::derive_filename(None, &res.final_url, None)
                });
                (
                    derived,
                    Some(res.final_url.to_string()),
                    res.content_length,
                    res.etag,
                    res.last_modified,
                    res.content_type,
                )
            }
            Err(_) => {
                let derived = sayl_http::filename::derive_filename(None, &parsed_url, None);
                (derived, None, None, None, None, None)
            }
        };

        let safe_filename = self.unique_filename_in(&self.inner.download_dir, &filename, None);

        let task_id = TaskId::new();
        let mut record =
            TaskRecord::with_id(task_id, url_str, &self.inner.download_dir, &safe_filename);
        record.final_url = final_url;
        record.total_size = total_size;
        record.etag = etag;
        record.last_modified = last_modified;
        record.content_type = content_type;
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

    /// Adds a download task with pre-supplied metadata (filename, size, referrer, headers).
    ///
    /// When both `filename` and `size` are supplied, the network probe is skipped entirely,
    /// enabling fast queueing without network roundtrips.
    /// In accordance with privacy rules, request headers and cookies are kept in memory only
    /// and are never persisted in the storage journal.
    pub async fn add_with_metadata(
        &self,
        url: impl AsRef<str>,
        filename: Option<String>,
        size: Option<u64>,
        referrer: Option<String>,
        headers: Option<Vec<(String, String)>>,
    ) -> Result<TaskId, CoreError> {
        let url_str = url.as_ref();
        let parsed_url =
            Url::parse(url_str).map_err(|e| CoreError::validation("url", e.to_string()))?;

        let mut custom_headers = headers.unwrap_or_default();
        if let Some(ref ref_url) = referrer {
            if !custom_headers
                .iter()
                .any(|(k, _)| k.eq_ignore_ascii_case("Referer"))
            {
                custom_headers.push(("Referer".to_string(), ref_url.clone()));
            }
        }

        let task_id = TaskId::new();

        let (final_filename, final_url, total_size, etag, last_modified, content_type) =
            if let (Some(f), Some(s)) = (filename, size) {
                let sanitized = sayl_http::filename::sanitize_filename(&f);
                (
                    sanitized,
                    Some(url_str.to_string()),
                    Some(s),
                    None,
                    None,
                    None,
                )
            } else {
                let header_map =
                    sayl_http::headers::build_request_headers(&custom_headers).unwrap_or_default();
                let probe_opts = ProbeOptions {
                    custom_headers: header_map,
                    ..Default::default()
                };
                let probe_res = sayl_http::probe::probe_with_options(
                    &self.inner.client,
                    parsed_url.clone(),
                    probe_opts,
                )
                .await;

                match probe_res {
                    Ok(res) => {
                        let derived = res.filename.clone().unwrap_or_else(|| {
                            sayl_http::filename::derive_filename(None, &res.final_url, None)
                        });
                        (
                            derived,
                            Some(res.final_url.to_string()),
                            res.content_length,
                            res.etag,
                            res.last_modified,
                            res.content_type,
                        )
                    }
                    Err(_) => {
                        let derived = sayl_http::filename::derive_filename(None, &parsed_url, None);
                        (derived, None, None, None, None, None)
                    }
                }
            };

        if !custom_headers.is_empty() {
            if let Ok(mut map) = self.inner.task_headers.lock() {
                map.insert(task_id, custom_headers);
            }
        }

        let safe_filename =
            self.unique_filename_in(&self.inner.download_dir, &final_filename, None);

        let mut record =
            TaskRecord::with_id(task_id, url_str, &self.inner.download_dir, &safe_filename);
        record.final_url = final_url;
        record.total_size = total_size;
        record.etag = etag;
        record.last_modified = last_modified;
        record.content_type = content_type;
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

    /// Returns the active stall timeout duration.
    pub fn stall_timeout(&self) -> Duration {
        Duration::from_millis(self.inner.stall_timeout_ms.load(Ordering::Relaxed))
    }

    /// Overrides the stall timeout duration (useful in automated tests to avoid long sleeps).
    pub fn set_stall_timeout(&self, timeout: Duration) {
        self.inner
            .stall_timeout_ms
            .store(timeout.as_millis() as u64, Ordering::Relaxed);
    }

    /// Retrieves in-memory request headers configured for a task.
    fn get_task_headers(&self, task_id: TaskId) -> Vec<(String, String)> {
        self.inner
            .task_headers
            .lock()
            .ok()
            .and_then(|m| m.get(&task_id).cloned())
            .unwrap_or_default()
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
        let mut record =
            self.inner.store.get_task(task_id)?.ok_or_else(|| {
                CoreError::storage("not_found", format!("task {task_id} not found"))
            })?;

        // If in Failed state, allow retry! Reset the state to Queued, clear previous error fields, and proceed with download
        if record.state == DownloadState::Failed {
            self.remove_scheduler_id(task_id);
            record.error = None;
            record.error_kind = None;
            self.transition_and_persist(&mut record, DownloadState::Queued)
                .await?;
        } else if record.state.is_terminal() {
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
        let res = self.run_download(task_id, record, sched_id).await;
        if res.is_err() {
            let promoted = self.finish_and_get_promoted(sched_id, QueueTaskStatus::Failed);
            self.spawn_promoted_tasks(promoted);
            self.remove_scheduler_id(task_id);
        }
        res
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
        // Register active task control (no semaphore needed - scheduler manages concurrency)
        let cancel_token = CancellationToken::new();
        let pause_token = CancellationToken::new();
        // Live per-task speed cap (bytes/sec, 0 = unlimited); UI edits update
        // this cell and the running transfer picks them up on the next chunk.
        let speed_limit = self.speed_limit_cell(task_id, record.speed_limit_kbps);
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

        // Resume / destination checks
        let mut safe_filename = sayl_http::filename::sanitize_filename(&record.filename);
        if record.filename != safe_filename {
            record.filename = safe_filename.clone();
            let _ = self.inner.store.upsert_task(&record);
        }
        let mut part_filename = format!("{safe_filename}.part");
        let base_dir = record.save_path.clone();
        let mut part_path = self.resolve_destination_path_in(&base_dir, &part_filename)?;
        let mut disk_len = if part_path.exists() {
            tokio::fs::metadata(&part_path)
                .await
                .map(|m| m.len())
                .unwrap_or(0)
        } else {
            0
        };

        let final_path = self.resolve_destination_path_in(&base_dir, &safe_filename)?;
        let final_len = if final_path.exists() {
            tokio::fs::metadata(&final_path)
                .await
                .map(|m| m.len())
                .unwrap_or(0)
        } else {
            0
        };

        let checkpoint = self
            .inner
            .store
            .load_checkpoint(task_id)
            .map_err(CoreError::from)?;
        let (checkpoint_written, checkpoint_ranges) = if let Some(ref cp) = checkpoint {
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

        // CRITICAL: a `.part` file is preallocated to `total_size` by the parallel engine
        // (`set_len(total_size)`), so `disk_len == total_size` is TRUE from the very first
        // moment of every parallel download and says nothing about real progress.
        // Completion must be proven by DURABLE evidence, never by file length alone:
        //   * the final (renamed) file is present and full-length, or
        //   * durable checkpoint ranges actually cover the whole `[0, total_size)` span.
        // Otherwise a paused task (part file preallocated, only 12% written) would look
        // "already finished" on the next `start()` and be marked Completed at 12.6%.
        let total_known = record.total_size.filter(|t| *t > 0);
        let checkpoint_covers_all = match total_known {
            Some(total) => {
                let contiguous = if checkpoint_ranges.is_empty() {
                    0
                } else {
                    contiguous_prefix(&checkpoint_ranges)
                };
                contiguous == total
            }
            None => false,
        };

        let is_already_finished = match total_known {
            Some(total) => {
                (final_len == total) || (disk_len == total && disk_len > 0 && checkpoint_covers_all)
            }
            None => false,
        };

        let contiguous_cp = if checkpoint_ranges.is_empty() {
            checkpoint_written
        } else {
            contiguous_prefix(&checkpoint_ranges)
        };

        // `disk_len` is NOT usable as a progress signal for preallocated `.part` files
        // (the parallel engine calls `set_len(total_size)` up front, so the file is
        // full-length on disk while holding almost no real data). Only durable
        // checkpoint evidence counts; `disk_len` merely caps what we claim as durable.
        let covered_bytes = if is_already_finished {
            record.total_size.unwrap()
        } else if contiguous_cp > 0 {
            contiguous_cp.min(disk_len)
        } else {
            0
        };

        let has_progress = (checkpoint.is_some() && checkpoint_written > 0)
            || record.written > 0
            || record.state == DownloadState::Paused;

        // Fast resume is allowed from Paused or Queued, with or without a validator!
        let can_fast_resume = matches!(record.state, DownloadState::Paused | DownloadState::Queued)
            && record.total_size.is_some()
            && covered_bytes > 0
            && has_progress;

        // A parallel pause can leave completed chunks scattered with a HOLE at the start,
        // so the contiguous prefix is 0 even though real bytes exist. Such a task cannot
        // "fast resume" (we need a contiguous prefix to resume from), and it must not walk
        // into the Probing path while `written > 0` because `transition_for_task` rejects
        // Probing once progress is recorded. Honest recovery: restart this task from zero.
        let needs_restart_from_zero = !can_fast_resume
            && record.total_size.is_some()
            && record.written > 0
            && covered_bytes == 0
            && matches!(record.state, DownloadState::Paused | DownloadState::Queued);

        if needs_restart_from_zero {
            let f = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&part_path)
                .map_err(|e| CoreError::io_with_context(e, "truncate part file for restart"))?;
            drop(f);
            record.written = 0;
            let _ = self.inner.store.upsert_task(&record);
            let _ = self.inner.store.save_checkpoint(&Checkpoint::new(
                task_id,
                0,
                record.total_size,
                Vec::new(),
            ));
        }

        let custom_headers = self.get_task_headers(task_id);
        let custom_headers_ref = if custom_headers.is_empty() {
            None
        } else {
            Some(custom_headers.as_slice())
        };

        if can_fast_resume {
            let resume_from = covered_bytes;
            if disk_len > resume_from {
                let f = std::fs::OpenOptions::new()
                    .write(true)
                    .open(&part_path)
                    .map_err(|e| CoreError::io_with_context(e, "open part file for truncation"))?;
                f.set_len(resume_from).map_err(|e| {
                    CoreError::io_with_context(e, "truncate part file to resume offset")
                })?;
            }

            let resume_info = ResumeInfo {
                url: record.url.clone(),
                etag: record.etag.clone(),
                last_modified: record.last_modified.clone(),
                total_size: record.total_size,
                written: resume_from,
            };

            let already_complete = is_already_finished || record.total_size == Some(resume_from);
            let mut outcome_status = QueueTaskStatus::Finished;

            // When recovery decides the transfer is already complete, record the verified
            // byte count so `complete_download` has honest evidence to validate against.
            // Without this the completion gate would see a stale `written` (e.g. a task that
            // crashed after its final byte but before the state was persisted) and refuse.
            if already_complete {
                if let Some(total) = record.total_size {
                    record.written = record.written.max(total);
                }
            }

            self.transition_and_persist(&mut record, DownloadState::Downloading)
                .await?;

            if !already_complete {
                let target_url = Url::parse(record.final_url.as_deref().unwrap_or(&record.url))
                    .map_err(|e| CoreError::validation("url", e.to_string()))?;

                let stall_timeout = self.stall_timeout();
                let mut stall_retries_remaining = 1;
                let mut current_offset = resume_from;
                let mut current_resume_info = Some(resume_info);

                outcome_status = loop {
                    let seq_result = crate::sequential::run_sequential_download(
                        &self.inner.client,
                        &target_url,
                        &part_path,
                        task_id,
                        record.total_size,
                        current_resume_info.as_ref(),
                        current_offset,
                        cancel_token.clone(),
                        pause_token.clone(),
                        &self.inner.event_sender,
                        &self.inner.store,
                        stall_timeout,
                        custom_headers_ref,
                        Some(speed_limit.clone()),
                    )
                    .await;

                    match seq_result {
                        Ok(DownloadOutcome::Completed { written }) => {
                            record.written = written;
                            self.inner.store.upsert_task(&record)?;
                            break QueueTaskStatus::Finished;
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
                            break QueueTaskStatus::Cancelled;
                        }
                        Err(err)
                            if err.error_kind() == "timeout" && stall_retries_remaining > 0 =>
                        {
                            stall_retries_remaining -= 1;
                            tracing::warn!(
                                "Download stalled: retrying once from current offset..."
                            );
                            let latest_cp =
                                self.inner.store.load_checkpoint(task_id).ok().flatten();
                            let current_disk = tokio::fs::metadata(&part_path)
                                .await
                                .map(|m| m.len())
                                .unwrap_or(0);
                            current_offset = if let Some(ref cp) = latest_cp {
                                if cp.range_map.is_empty() {
                                    cp.written.min(current_disk)
                                } else {
                                    contiguous_prefix(&cp.range_map).min(current_disk)
                                }
                            } else {
                                current_disk
                            };
                            if current_disk > current_offset {
                                if let Ok(f) =
                                    std::fs::OpenOptions::new().write(true).open(&part_path)
                                {
                                    let _ = f.set_len(current_offset);
                                }
                            }
                            current_resume_info = Some(ResumeInfo {
                                url: record.url.clone(),
                                etag: record.etag.clone(),
                                last_modified: record.last_modified.clone(),
                                total_size: record.total_size,
                                written: current_offset,
                            });
                            continue;
                        }
                        Err(err) => {
                            let _ = self
                                .transition_and_persist(&mut record, DownloadState::Failed)
                                .await;
                            let err_msg = match &err {
                                CoreError::Http {
                                    status_code: Some(code),
                                    message,
                                    ..
                                } => {
                                    if message.starts_with("HTTP ") {
                                        message.clone()
                                    } else {
                                        format!("HTTP {code} {message}")
                                    }
                                }
                                _ => err.to_string(),
                            };
                            record.error = Some(err_msg.clone());
                            record.error_kind = Some(err.error_kind().to_string());
                            let _ = self.inner.store.upsert_task(&record);
                            let _ = self
                                .inner
                                .event_sender
                                .emit(
                                    task_id,
                                    EventKind::Error,
                                    EventPayload::Error {
                                        error: err_msg,
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
            }

            if matches!(outcome_status, QueueTaskStatus::Finished) || already_complete {
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

            return Ok(());
        }

        // Transition to Probing for new or non-resumable downloads
        if let Err(err) = sayl_core::transition_for_task(
            record.state,
            DownloadState::Probing,
            record.total_size,
            record.written,
        ) {
            let promoted = self.finish_and_get_promoted(sched_id, QueueTaskStatus::Failed);
            self.spawn_promoted_tasks(promoted);
            self.remove_scheduler_id(task_id);
            return Err(CoreError::from(err));
        }

        if let Err(err) = self
            .transition_and_persist(&mut record, DownloadState::Probing)
            .await
        {
            let promoted = self.finish_and_get_promoted(sched_id, QueueTaskStatus::Failed);
            self.spawn_promoted_tasks(promoted);
            self.remove_scheduler_id(task_id);
            return Err(err);
        }

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

        let probe_hdr_map =
            sayl_http::headers::build_request_headers(&custom_headers).unwrap_or_default();
        let probe_opts = ProbeOptions {
            custom_headers: probe_hdr_map,
            ..Default::default()
        };

        let probe_res =
            match sayl_http::probe::probe_with_options(&self.inner.client, target_url, probe_opts)
                .await
            {
                Ok(res) => res,
                Err(err) => {
                    let _ = self
                        .transition_and_persist(&mut record, DownloadState::Failed)
                        .await;
                    let core_err = CoreError::from(err);
                    let err_msg = match &core_err {
                        CoreError::Http {
                            status_code: Some(code),
                            message,
                            ..
                        } => {
                            if message.starts_with("HTTP ") {
                                message.clone()
                            } else {
                                format!("HTTP {code} {message}")
                            }
                        }
                        _ => core_err.to_string(),
                    };
                    record.error = Some(err_msg.clone());
                    record.error_kind = Some(core_err.error_kind().to_string());
                    let _ = self.inner.store.upsert_task(&record);
                    let _ = self
                        .inner
                        .event_sender
                        .emit(
                            task_id,
                            EventKind::Error,
                            EventPayload::Error {
                                error: err_msg,
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

        let candidate_filename = probe_res
            .filename
            .clone()
            .unwrap_or_else(|| record.filename.clone());

        // Check if candidate_filename collides with existing disk files or other active tasks.
        let is_taken = {
            let target_file = base_dir.join(&candidate_filename);
            let other_task_active = self
                .inner
                .store
                .list_tasks()
                .unwrap_or_default()
                .into_iter()
                .any(|t| {
                    t.id != task_id
                        && !t.state.is_terminal()
                        && same_folder_path(&t.save_path, &base_dir)
                        && same_filename_case(&t.filename, &candidate_filename)
                });
            let part_collides = candidate_filename != record.filename
                && base_dir.join(format!("{candidate_filename}.part")).exists();

            target_file.exists() || other_task_active || part_collides
        };

        if is_taken {
            let unique_name =
                self.unique_filename_in(&base_dir, &candidate_filename, Some(task_id));
            if unique_name != record.filename {
                record.filename = unique_name;
                safe_filename = sayl_http::filename::sanitize_filename(&record.filename);
                part_filename = format!("{safe_filename}.part");
                part_path = self.resolve_destination_path_in(&base_dir, &part_filename)?;
                // The new name is free by construction, so its `.part` file cannot
                // hold any bytes: drop the length measured for the old name so the
                // resume decision below never reads a stale partial length.
                disk_len = 0;
            }
        }
        self.inner.store.upsert_task(&record)?;

        let mut resume_from = 0u64;
        let mut already_complete = false;
        let mut resume_info_opt = None;
        let resume_written = covered_bytes;

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
                    // Record verified bytes as completion evidence for the guard in
                    // `complete_download` (which refuses to mark Completed on a short count).
                    if let Some(total) = record.total_size {
                        record.written = record.written.max(total);
                        let _ = self.inner.store.upsert_task(&record);
                    }
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
                    Some(speed_limit.clone()),
                )
                .await;

                outcome_status = match download_result {
                    Ok((DownloadOutcome::Completed { written }, _report)) => {
                        record.written = written;
                        self.inner.store.upsert_task(&record)?;
                        QueueTaskStatus::Finished
                    }
                    Ok((DownloadOutcome::Paused { written }, report)) => {
                        record.written = written;
                        // `parallel.rs` already persisted an accurate checkpoint built from
                        // the lease table's completed ranges. Do NOT overwrite it with the
                        // stale pre-run `checkpoint_ranges`: for a parallel transfer the
                        // completed ranges are NOT a contiguous prefix, so replacing them
                        // would make the next resume treat unwritten holes as downloaded
                        // and corrupt the file.
                        // LeaseReport stores (start, end_inclusive); Checkpoint expects (start, end_exclusive)
                        let paused_ranges: Vec<(u64, u64)> = report
                            .completed_ranges
                            .iter()
                            .map(|&(s, e)| (s, e + 1)) // inclusive → exclusive
                            .collect();
                        if !paused_ranges.is_empty() {
                            let checkpoint =
                                Checkpoint::new(task_id, written, record.total_size, paused_ranges);
                            let _ = self.inner.store.save_checkpoint(&checkpoint);
                        }
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
                        let should_fallback = err.error_kind() == "range_unsupported"
                            || err.error_kind() == "http_429"
                            || err.error_kind() == "http_503"
                            || err.error_kind() == "network"
                            || err.error_kind() == "timeout"
                            || matches!(err.status_code(), Some(429 | 503 | 416))
                            || matches!(err, CoreError::Io { .. })
                            || err.to_string().to_ascii_lowercase().contains("incomplete");

                        if should_fallback {
                            tracing::warn!(
                                "parallel download failed ({err}), falling back smoothly to sequential"
                            );
                            let latest_cp =
                                self.inner.store.load_checkpoint(task_id).ok().flatten();
                            let contiguous = latest_cp
                                .as_ref()
                                .map(|c| contiguous_prefix(&c.range_map))
                                .unwrap_or(0);
                            let current_disk_len = tokio::fs::metadata(&part_path)
                                .await
                                .map(|m| m.len())
                                .unwrap_or(0);
                            let fallback_offset = contiguous.max(resume_from).min(current_disk_len);

                            let fallback_resume_info =
                                resume_info_opt.clone().unwrap_or_else(|| ResumeInfo {
                                    url: record.url.clone(),
                                    etag: record.etag.clone(),
                                    last_modified: record.last_modified.clone(),
                                    total_size: record.total_size,
                                    written: fallback_offset,
                                });

                            let seq_result = crate::sequential::run_sequential_download(
                                &self.inner.client,
                                &probe_res.final_url,
                                &part_path,
                                task_id,
                                record.total_size,
                                Some(&fallback_resume_info),
                                fallback_offset,
                                cancel_token.clone(),
                                pause_token.clone(),
                                &self.inner.event_sender,
                                &self.inner.store,
                                self.stall_timeout(),
                                custom_headers_ref,
                                Some(speed_limit.clone()),
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
                                    let err_msg = match &seq_err {
                                        CoreError::Http {
                                            status_code: Some(code),
                                            message,
                                            ..
                                        } => {
                                            if message.starts_with("HTTP ") {
                                                message.clone()
                                            } else {
                                                format!("HTTP {code} {message}")
                                            }
                                        }
                                        _ => seq_err.to_string(),
                                    };
                                    record.error = Some(err_msg.clone());
                                    record.error_kind = Some(seq_err.error_kind().to_string());
                                    let _ = self.inner.store.upsert_task(&record);
                                    let _ = self
                                        .inner
                                        .event_sender
                                        .emit(
                                            task_id,
                                            EventKind::Error,
                                            EventPayload::Error {
                                                error: err_msg,
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
                            let err_msg = match &err {
                                CoreError::Http {
                                    status_code: Some(code),
                                    message,
                                    ..
                                } => {
                                    if message.starts_with("HTTP ") {
                                        message.clone()
                                    } else {
                                        format!("HTTP {code} {message}")
                                    }
                                }
                                _ => err.to_string(),
                            };
                            record.error = Some(err_msg.clone());
                            record.error_kind = Some(err.error_kind().to_string());
                            let _ = self.inner.store.upsert_task(&record);
                            let _ = self
                                .inner
                                .event_sender
                                .emit(
                                    task_id,
                                    EventKind::Error,
                                    EventPayload::Error {
                                        error: err_msg,
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
                let stall_timeout = self.stall_timeout();
                let mut stall_retries_remaining = 1;
                let mut current_offset = resume_from;
                let mut current_resume_info = resume_info_opt.clone();

                outcome_status = loop {
                    let download_result = crate::sequential::run_sequential_download(
                        &self.inner.client,
                        &probe_res.final_url,
                        &part_path,
                        task_id,
                        record.total_size,
                        current_resume_info.as_ref(),
                        current_offset,
                        cancel_token.clone(),
                        pause_token.clone(),
                        &self.inner.event_sender,
                        &self.inner.store,
                        stall_timeout,
                        custom_headers_ref,
                        Some(speed_limit.clone()),
                    )
                    .await;

                    match download_result {
                        Ok(DownloadOutcome::Completed { written }) => {
                            record.written = written;
                            self.inner.store.upsert_task(&record)?;
                            break QueueTaskStatus::Finished;
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
                            break QueueTaskStatus::Cancelled;
                        }
                        Err(err)
                            if err.error_kind() == "timeout" && stall_retries_remaining > 0 =>
                        {
                            stall_retries_remaining -= 1;
                            tracing::warn!(
                                "Download stalled: retrying once from current offset..."
                            );
                            let latest_cp =
                                self.inner.store.load_checkpoint(task_id).ok().flatten();
                            let current_disk = tokio::fs::metadata(&part_path)
                                .await
                                .map(|m| m.len())
                                .unwrap_or(0);
                            current_offset = if let Some(ref cp) = latest_cp {
                                if cp.range_map.is_empty() {
                                    cp.written.min(current_disk)
                                } else {
                                    contiguous_prefix(&cp.range_map).min(current_disk)
                                }
                            } else {
                                current_disk
                            };
                            if current_disk > current_offset {
                                if let Ok(f) =
                                    std::fs::OpenOptions::new().write(true).open(&part_path)
                                {
                                    let _ = f.set_len(current_offset);
                                }
                            }
                            current_resume_info = Some(ResumeInfo {
                                url: record.url.clone(),
                                etag: record.etag.clone(),
                                last_modified: record.last_modified.clone(),
                                total_size: record.total_size,
                                written: current_offset,
                            });
                            continue;
                        }
                        Err(err) => {
                            let _ = self
                                .transition_and_persist(&mut record, DownloadState::Failed)
                                .await;
                            let err_msg = match &err {
                                CoreError::Http {
                                    status_code: Some(code),
                                    message,
                                    ..
                                } => {
                                    if message.starts_with("HTTP ") {
                                        message.clone()
                                    } else {
                                        format!("HTTP {code} {message}")
                                    }
                                }
                                _ => err.to_string(),
                            };
                            record.error = Some(err_msg.clone());
                            record.error_kind = Some(err.error_kind().to_string());
                            let _ = self.inner.store.upsert_task(&record);
                            let _ = self
                                .inner
                                .event_sender
                                .emit(
                                    task_id,
                                    EventKind::Error,
                                    EventPayload::Error {
                                        error: err_msg,
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

        let final_path =
            self.resolve_destination_path_in(record.save_path.as_path(), &record.filename)?;

        let actual_len = if part_path.exists() {
            tokio::fs::metadata(part_path)
                .await
                .map_err(|e| CoreError::io_with_context(e, "get part file metadata"))?
                .len()
        } else if final_path.exists() {
            tokio::fs::metadata(&final_path)
                .await
                .map_err(|e| CoreError::io_with_context(e, "get final file metadata"))?
                .len()
        } else {
            0
        };

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

        // INVARIANT: `Completed` requires downloaded == total.
        //
        // `actual_len` alone is NOT sufficient proof: the parallel engine preallocates
        // the `.part` file to `total_size`, so a 12.6%-complete file already reports the
        // full length. The only trustworthy counter is `record.written`, which each caller
        // sets from the transfer's own `DownloadOutcome::Completed { written }` (or from a
        // durably-verified recovery). Refuse completion whenever that counter falls short.
        let completion_provable = match record.total_size {
            Some(total) => total > 0 && record.written >= total,
            None => record.written > 0 || actual_len > 0,
        };
        if !completion_provable {
            let _ = self
                .transition_and_persist(record, DownloadState::Failed)
                .await;
            record.error = Some("completion refused: incomplete body".to_string());
            record.error_kind = Some("integrity".to_string());
            let _ = self.inner.store.upsert_task(record);
            return Ok(QueueTaskStatus::Failed);
        }

        // Finalizing
        self.transition_and_persist(record, DownloadState::Finalizing)
            .await?;

        if part_path.exists() {
            if final_path.exists() {
                let _ = tokio::fs::remove_file(&final_path).await;
            }
            tokio::fs::rename(part_path, &final_path)
                .await
                .map_err(|e| CoreError::io_with_context(e, "rename part file to final file"))?;
        }

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
            // Task is queued but not running: remove from scheduler and mark as Paused directly via Pausing
            if let Some(sched_id) = self.get_scheduler_id(task_id) {
                self.scheduler_pause_queued(sched_id);
            }
            if self
                .transition_and_persist(&mut record, DownloadState::Pausing)
                .await
                .is_ok()
            {
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
            if sayl_core::transition_for_task(
                record.state,
                DownloadState::Cancelling,
                record.total_size,
                record.written,
            )
            .is_ok()
            {
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

            let part_path = task.save_path.join(format!("{}.part", task.filename));
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
            let final_path = task.save_path.join(&task.filename);
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

            // If no valid checkpoint exists but part file has data (e.g., sequential crash mid-transfer),
            // create a checkpoint assuming contiguous data from 0. This enables proper resume with Range header.
            // Guard: the parallel engine preallocates `.part` to `total_size`, so a file that
            // already spans the full size is NOT proof of a contiguous prefix (it may contain
            // unwritten holes). Only trust `part_len` when it is strictly shorter than total.
            let part_is_sequential_prefix = task.total_size.is_none_or(|t| part_len < t);
            if !checkpoint_valid && part_len > 0 && !final_complete && part_is_sequential_prefix {
                let new_checkpoint = Checkpoint::new(
                    task.id,
                    part_len,
                    task.total_size,
                    vec![(0, part_len)], // Sequential downloads always write contiguously from 0
                );
                self.inner
                    .store
                    .save_checkpoint(&new_checkpoint)
                    .map_err(CoreError::from)?;
                checkpoint_valid = true;
                checkpoint_written = part_len;
                _checkpoint_ranges = vec![(0, part_len)];
                // Also update task.written so it matches the checkpoint
                if part_len > task.written {
                    task.written = part_len;
                    self.inner.store.upsert_task(&task)?;
                }
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
    /// Updates filename and destination folder for a queued task (pre-download confirm).
    pub fn update_queued_destination(
        &self,
        task_id: TaskId,
        save_path: PathBuf,
        filename: String,
    ) -> Result<(), CoreError> {
        let mut record = self
            .get_task(task_id)?
            .ok_or_else(|| CoreError::validation("task_id", "task not found"))?;
        if record.state != DownloadState::Queued {
            return Err(CoreError::validation(
                "state",
                "task must be queued to update destination",
            ));
        }
        let safe_filename = sayl_http::filename::sanitize_filename(&filename);
        if safe_filename.is_empty() {
            return Err(CoreError::validation("filename", "filename is empty"));
        }
        let unique_filename = self.unique_filename_in(&save_path, &safe_filename, Some(task_id));
        std::fs::create_dir_all(&save_path)
            .map_err(|e| CoreError::io_with_context(e, "create save_path"))?;
        record.filename = unique_filename;
        record.save_path = save_path;
        record.updated_at = Utc::now();
        self.inner.store.upsert_task(&record)?;
        Ok(())
    }

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

    /// Returns the live per-connection snapshot of a running parallel transfer.
    ///
    /// Each entry names one connection slot with the byte range it owns, the
    /// bytes written inside that range and its coarse state. The list is empty
    /// when the task is not transferring in parallel, so callers may read it
    /// unconditionally while building a task view.
    pub fn connection_snapshot(&self, task_id: TaskId) -> Vec<SegmentRange> {
        crate::parallel::connection_snapshot(task_id)
    }

    /// Deletes a task from the store.
    pub fn delete_task(&self, task_id: TaskId) -> Result<(), CoreError> {
        self.remove_scheduler_id(task_id);
        if let Ok(mut map) = self.inner.task_headers.lock() {
            map.remove(&task_id);
        }
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
        let new_state =
            sayl_core::transition_for_task(old_state, target, record.total_size, record.written)?;
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

    /// Resolves a safe destination path within the engine default download directory.
    pub fn resolve_destination_path(&self, raw_filename: &str) -> Result<PathBuf, CoreError> {
        self.resolve_destination_path_in(&self.inner.download_dir, raw_filename)
    }

    /// Resolves a safe destination path within `base_dir` from an untrusted filename.
    pub fn resolve_destination_path_in(
        &self,
        base_dir: &Path,
        raw_filename: &str,
    ) -> Result<PathBuf, CoreError> {
        let safe_filename = sayl_http::filename::sanitize_filename(raw_filename);
        let target_path = base_dir.join(&safe_filename);
        if !target_path.starts_with(base_dir) {
            return Err(CoreError::scheduler(format!(
                "destination path traversal detected: '{raw_filename}' escapes download directory"
            )));
        }
        Ok(target_path)
    }

    /// Checks whether `filename` is taken in `dir` by either:
    /// 1. A physical file `<dir>/<filename>` on disk
    /// 2. A physical `.part` file `<dir>/<filename>.part` on disk
    /// 3. An active or queued download in the store targeting `<dir>/<filename>` (excluding `exclude_task`).
    pub fn is_filename_taken_in(
        &self,
        dir: &Path,
        filename: &str,
        exclude_task: Option<TaskId>,
    ) -> bool {
        let safe_name = sayl_http::filename::sanitize_filename(filename);
        let target_file = dir.join(&safe_name);
        if target_file.exists() {
            return true;
        }
        let part_file = dir.join(format!("{safe_name}.part"));
        if part_file.exists() {
            return true;
        }

        if let Ok(tasks) = self.inner.store.list_tasks() {
            for t in tasks {
                if Some(t.id) == exclude_task || t.state.is_terminal() {
                    continue;
                }
                if same_folder_path(&t.save_path, dir)
                    && same_filename_case(&t.filename, &safe_name)
                {
                    return true;
                }
            }
        }

        false
    }

    /// Resolves a unique, non-colliding filename in `dir` (IDM style),
    /// appending sequential numbers `(1)`, `(2)`, ... when a collision is detected.
    pub fn unique_filename_in(
        &self,
        dir: &Path,
        base_name: &str,
        exclude_task: Option<TaskId>,
    ) -> String {
        let active_names: Vec<String> = self
            .inner
            .store
            .list_tasks()
            .unwrap_or_default()
            .into_iter()
            .filter(|t| {
                Some(t.id) != exclude_task
                    && !t.state.is_terminal()
                    && same_folder_path(&t.save_path, dir)
            })
            .map(|t| t.filename)
            .collect();

        sayl_http::filename::generate_unique_filename(base_name, |candidate| {
            let target_file = dir.join(candidate);
            if target_file.exists() {
                return true;
            }
            let part_file = dir.join(format!("{candidate}.part"));
            if part_file.exists() {
                return true;
            }
            active_names
                .iter()
                .any(|f| same_filename_case(f, candidate))
        })
    }

    /// Resolves a unique filename in `dir` without excluding any task.
    pub fn unique_filename(&self, dir: &Path, base_name: &str) -> String {
        self.unique_filename_in(dir, base_name, None)
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

fn same_folder_path(left: &Path, right: &Path) -> bool {
    if let (Ok(c1), Ok(c2)) = (left.canonicalize(), right.canonicalize()) {
        return c1 == c2;
    }
    let norm = |p: &Path| {
        p.to_string_lossy()
            .trim_end_matches(['/', '\\'])
            .replace('/', "\\")
            .to_lowercase()
    };
    norm(left) == norm(right)
}

fn same_filename_case(left: &str, right: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        left.eq_ignore_ascii_case(right)
    }
    #[cfg(not(target_os = "windows"))]
    {
        left == right
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

    /// Creates an engine plus a scratch folder that plays the role of the
    /// destination directory for the collision tests.
    fn make_collision_engine() -> (tempfile::TempDir, Engine) {
        let temp_dir = make_test_temp_dir();
        let engine =
            Engine::open_in_memory(temp_dir.path(), EngineConfig::default()).expect("open engine");
        (temp_dir, engine)
    }

    fn write_file(path: &Path, len: usize) {
        std::fs::write(path, vec![0u8; len]).expect("write fixture file");
    }

    #[test]
    fn test_unique_filename_in_avoids_existing_disk_file() {
        let (temp_dir, engine) = make_collision_engine();
        let dir = temp_dir.path();

        // No file yet: the requested name is used unchanged.
        assert_eq!(
            engine.unique_filename_in(dir, "Zed-x86_64.exe", None),
            "Zed-x86_64.exe"
        );

        // Existing final file forces the IDM-style "(1)" suffix.
        write_file(&dir.join("Zed-x86_64.exe"), 16);
        assert_eq!(
            engine.unique_filename_in(dir, "Zed-x86_64.exe", None),
            "Zed-x86_64 (1).exe"
        );

        // Existing "(1)" increments instead of nesting: "(2)".
        write_file(&dir.join("Zed-x86_64 (1).exe"), 16);
        assert_eq!(
            engine.unique_filename_in(dir, "Zed-x86_64.exe", None),
            "Zed-x86_64 (2).exe"
        );
    }

    #[test]
    fn test_unique_filename_in_avoids_existing_part_file() {
        let (temp_dir, engine) = make_collision_engine();
        let dir = temp_dir.path();

        write_file(&dir.join("archive.tar.gz.part"), 32);
        assert_eq!(
            engine.unique_filename_in(dir, "archive.tar.gz", None),
            "archive.tar (1).gz"
        );
    }

    #[test]
    fn test_unique_filename_in_avoids_active_task_target() {
        let (temp_dir, engine) = make_collision_engine();
        let dir = temp_dir.path();

        // A queued (non-terminal) task already targets "<dir>/report.pdf".
        let other = TaskRecord::with_id(
            TaskId::new(),
            "http://example.com/report.pdf",
            dir,
            "report.pdf",
        );
        engine.inner.store.upsert_task(&other).expect("upsert task");

        assert_eq!(
            engine.unique_filename_in(dir, "report.pdf", None),
            "report (1).pdf"
        );

        // The owning task itself may keep its own name when excluded.
        assert_eq!(
            engine.unique_filename_in(dir, "report.pdf", Some(other.id)),
            "report.pdf"
        );

        // Terminal tasks stop holding the name (their disk file is gone here).
        let mut completed = other.clone();
        completed.state = DownloadState::Completed;
        engine
            .inner
            .store
            .upsert_task(&completed)
            .expect("upsert completed task");
        assert_eq!(
            engine.unique_filename_in(dir, "report.pdf", None),
            "report.pdf"
        );
    }

    #[test]
    fn test_unique_filename_in_edge_shapes_and_sequence() {
        let (temp_dir, engine) = make_collision_engine();
        let dir = temp_dir.path();

        // Names with multiple numbers: only the trailing "(N)" increments.
        write_file(&dir.join("file1_v2.0_test3 (1).exe"), 8);
        assert_eq!(
            engine.unique_filename_in(dir, "file1_v2.0_test3 (1).exe", None),
            "file1_v2.0_test3 (2).exe"
        );

        // Names without any extension.
        write_file(&dir.join("myfile"), 8);
        assert_eq!(engine.unique_filename_in(dir, "myfile", None), "myfile (1)");

        // Sequential increments (1), (2), (3) ... on one base name.
        write_file(&dir.join("seq.bin"), 8);
        for expected in ["seq (1).bin", "seq (2).bin", "seq (3).bin"] {
            let unique = engine.unique_filename_in(dir, "seq.bin", None);
            assert_eq!(unique, expected);
            write_file(&dir.join(&unique), 8);
        }

        // A foreground .part file of an unrelated crash also blocks the name.
        write_file(&dir.join("crashy.zip.part"), 8);
        assert_eq!(
            engine.unique_filename_in(dir, "crashy.zip", None),
            "crashy (1).zip"
        );
    }

    #[test]
    fn test_is_filename_taken_in_matches_resolution() {
        let (temp_dir, engine) = make_collision_engine();
        let dir = temp_dir.path();

        assert!(!engine.is_filename_taken_in(dir, "free.bin", None));
        write_file(&dir.join("free.bin"), 4);
        assert!(engine.is_filename_taken_in(dir, "free.bin", None));

        write_file(&dir.join("stale.bin.part"), 4);
        assert!(engine.is_filename_taken_in(dir, "stale.bin", None));
        assert!(!engine.is_filename_taken_in(&temp_dir.path().join("nope"), "stale.bin", None));
    }
}

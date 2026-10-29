//! Bounded parallel multi-connection download implementation using dynamic lease scheduling.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use sayl_core::error::CoreError;
use sayl_core::event::{EventKind, EventPayload, EventSender};
use sayl_core::id::TaskId;
use sayl_storage::{Checkpoint, Store};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::lease::{LeaseReport, LeaseTable, WorkerId, DEFAULT_CHUNK_SIZE, DEFAULT_LEASE_TTL};
use crate::queue::TokenBucket;
use crate::sequential::DownloadOutcome;

/// Options configuring parallel multi-connection transfers.
#[derive(Debug, Clone)]
pub struct ParallelOptions {
    /// Range chunk size in bytes.
    pub chunk_size: usize,
    /// Duration of a chunk lease before it is eligible for stealing.
    pub lease_ttl: Duration,
}

impl Default for ParallelOptions {
    fn default() -> Self {
        Self {
            chunk_size: DEFAULT_CHUNK_SIZE,
            lease_ttl: DEFAULT_LEASE_TTL,
        }
    }
}

/// Maximum number of per-connection entries published in one snapshot.
///
/// One row per active connection is all the downloading window needs; connections
/// beyond this cap are not published individually, so an absurd connection count
/// can never flood the webview. Their bytes are still counted in the aggregate
/// progress that drives the bar.
pub const MAX_SEGMENT_ENTRIES: usize = 64;

/// Coarse lifecycle state of one connection in a [`SegmentRange`] snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentState {
    /// The connection slot exists but does not own a byte range yet.
    #[default]
    Pending,
    /// The connection is streaming the range it owns.
    Downloading,
    /// The connection finished the range it owns.
    Done,
}

/// One connection of a running parallel transfer, as published to the frontend.
///
/// Monotonicity contract: while `start`/`end` stay unchanged for one `index`, the
/// `written` counter never decreases. A connection that moves on to another byte
/// range rebases `written` to `0`, and a new attempt starts a fresh snapshot, so
/// consumers detect both cases by comparing the range instead of assuming a
/// single global counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentRange {
    /// Zero-based connection slot index, stable for the whole attempt.
    pub index: u32,
    /// First byte owned by this connection (inclusive).
    pub start: u64,
    /// Last byte owned by this connection (exclusive).
    pub end: u64,
    /// Bytes written inside `[start, end)` during this attempt.
    pub written: u64,
    /// `true` once the whole `[start, end)` range is durably written.
    pub completed: bool,
    /// Coarse connection state: `pending`, `downloading`, or `done`.
    pub state: SegmentState,
}

/// Mutable state of one connection slot inside [`ConnectionRegistry`].
#[derive(Debug, Clone, Copy)]
struct SegmentSlot {
    start: u64,
    end: u64,
    written: u64,
    state: SegmentState,
}

impl SegmentSlot {
    /// Slot state before it claims any byte range.
    const PENDING: Self = Self {
        start: 0,
        end: 0,
        written: 0,
        state: SegmentState::Pending,
    };

    /// Length in bytes of the owned range.
    fn span(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }
}

/// Live per-connection progress of one running parallel transfer.
///
/// Workers publish on chunk boundaries; updates are lock-scoped and O(1), so the
/// 500 ms progress cadence is never delayed and no extra event is emitted per
/// chunk. [`connection_snapshot`] reads the current state on demand.
#[derive(Debug)]
struct ConnectionRegistry {
    slots: StdMutex<Vec<SegmentSlot>>,
}

impl ConnectionRegistry {
    /// Creates a registry with one slot per worker, capped at [`MAX_SEGMENT_ENTRIES`].
    fn new(workers: usize) -> Self {
        let count = workers.clamp(1, MAX_SEGMENT_ENTRIES);
        Self {
            slots: StdMutex::new(vec![SegmentSlot::PENDING; count]),
        }
    }

    /// Records that `worker` now owns `[start, end)`.
    ///
    /// Re-claiming the same range keeps the byte count so the published value
    /// stays monotonic; a different range starts that slot over.
    fn begin(&self, worker: WorkerId, start: u64, end: u64) {
        let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        let Some(slot) = slots.get_mut(worker) else {
            return;
        };
        if slot.start != start || slot.end != end {
            *slot = SegmentSlot {
                start,
                end,
                written: 0,
                state: SegmentState::Downloading,
            };
        } else {
            slot.state = SegmentState::Downloading;
        }
    }

    /// Adds `bytes` freshly written bytes to `worker`'s current range.
    fn advance(&self, worker: WorkerId, bytes: u64) {
        let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        let Some(slot) = slots.get_mut(worker) else {
            return;
        };
        let span = slot.span();
        slot.written = slot.written.saturating_add(bytes).min(span);
    }

    /// Marks `worker`'s current range as fully written.
    fn finish(&self, worker: WorkerId) {
        let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        let Some(slot) = slots.get_mut(worker) else {
            return;
        };
        let span = slot.span();
        slot.written = span;
        slot.state = SegmentState::Done;
    }

    /// Copies the current state, ordered by connection index.
    fn snapshot(&self) -> Vec<SegmentRange> {
        let slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        slots
            .iter()
            .enumerate()
            .map(|(index, slot)| SegmentRange {
                index: index as u32,
                start: slot.start,
                end: slot.end,
                written: slot.written,
                completed: slot.state == SegmentState::Done,
                state: slot.state,
            })
            .collect()
    }
}

/// Registries of the parallel transfers running in this process, keyed by task.
///
/// The snapshot is a process-local UI detail: keeping it here lets
/// [`connection_snapshot`] stay a plain synchronous accessor instead of adding
/// another argument to the public transfer entry points.
static ACTIVE_CONNECTIONS: OnceLock<StdMutex<HashMap<TaskId, Arc<ConnectionRegistry>>>> =
    OnceLock::new();

/// Shared map of live connection registries.
fn active_connections() -> &'static StdMutex<HashMap<TaskId, Arc<ConnectionRegistry>>> {
    ACTIVE_CONNECTIONS.get_or_init(|| StdMutex::new(HashMap::new()))
}

/// Returns the live per-connection snapshot of the parallel transfer for `task_id`.
///
/// The list holds one [`SegmentRange`] per connection slot (capped at
/// [`MAX_SEGMENT_ENTRIES`]) and is empty when the task is not running a parallel
/// transfer, so callers may read it unconditionally while building a task view.
pub fn connection_snapshot(task_id: TaskId) -> Vec<SegmentRange> {
    let registry = active_connections()
        .lock()
        .ok()
        .and_then(|map| map.get(&task_id).cloned());
    match registry {
        Some(registry) => registry.snapshot(),
        None => Vec::new(),
    }
}

/// Publishes one transfer's registry while it runs and withdraws it on every exit path.
struct SnapshotRegistration {
    task_id: TaskId,
    registry: Arc<ConnectionRegistry>,
}

impl SnapshotRegistration {
    /// Registers `registry` as the live snapshot of `task_id`.
    fn new(task_id: TaskId, registry: Arc<ConnectionRegistry>) -> Self {
        if let Ok(mut map) = active_connections().lock() {
            map.insert(task_id, Arc::clone(&registry));
        }
        Self { task_id, registry }
    }
}

impl Drop for SnapshotRegistration {
    fn drop(&mut self) {
        if let Ok(mut map) = active_connections().lock() {
            // Only withdraw our own entry: a newer attempt of the same task keeps its own.
            let ours = map
                .get(&self.task_id)
                .is_some_and(|current| Arc::ptr_eq(current, &self.registry));
            if ours {
                map.remove(&self.task_id);
            }
        }
    }
}

/// Executes a bounded parallel multi-connection download to a `.part` file.
#[allow(clippy::too_many_arguments)]
pub async fn run_parallel_download(
    client: &reqwest::Client,
    url: &Url,
    part_path: &Path,
    task_id: TaskId,
    total_size: u64,
    max_connections: usize,
    chunk_size: usize,
    cancel_token: CancellationToken,
    pause_token: CancellationToken,
    event_sender: &EventSender,
    store: Arc<Store>,
    speed_limit: Option<Arc<AtomicU64>>,
) -> Result<DownloadOutcome, CoreError> {
    let (outcome, _report) = run_parallel_download_with_options(
        client,
        url,
        part_path,
        task_id,
        total_size,
        max_connections,
        chunk_size,
        cancel_token,
        pause_token,
        event_sender,
        store,
        &[], // No completed ranges for fresh download
        DEFAULT_LEASE_TTL,
        speed_limit,
    )
    .await?;
    Ok(outcome)
}

/// Executes a parallel download and returns both the [`DownloadOutcome`] and the resulting [`LeaseReport`].
#[allow(clippy::too_many_arguments)]
pub async fn run_parallel_download_with_report(
    client: &reqwest::Client,
    url: &Url,
    part_path: &Path,
    task_id: TaskId,
    total_size: u64,
    max_connections: usize,
    chunk_size: usize,
    cancel_token: CancellationToken,
    pause_token: CancellationToken,
    event_sender: &EventSender,
    store: Arc<Store>,
    completed_ranges: &[(u64, u64)],
    lease_ttl: Duration,
    speed_limit: Option<Arc<AtomicU64>>,
) -> Result<(DownloadOutcome, LeaseReport), CoreError> {
    run_parallel_download_with_options(
        client,
        url,
        part_path,
        task_id,
        total_size,
        max_connections,
        chunk_size,
        cancel_token,
        pause_token,
        event_sender,
        store,
        completed_ranges,
        lease_ttl,
        speed_limit,
    )
    .await
}

/// Executes a parallel download with fully configurable [`ParallelOptions`].
///
/// While the transfer runs, a live per-connection snapshot is available through
/// [`connection_snapshot`]; it disappears as soon as this call returns, on both
/// the success and the failure path.
#[allow(clippy::too_many_arguments)]
pub async fn run_parallel_download_with_options(
    client: &reqwest::Client,
    url: &Url,
    part_path: &Path,
    task_id: TaskId,
    total_size: u64,
    max_connections: usize,
    chunk_size: usize,
    cancel_token: CancellationToken,
    pause_token: CancellationToken,
    event_sender: &EventSender,
    store: Arc<Store>,
    completed_ranges: &[(u64, u64)],
    lease_ttl: Duration,
    speed_limit: Option<Arc<AtomicU64>>,
) -> Result<(DownloadOutcome, LeaseReport), CoreError> {
    if cancel_token.is_cancelled() {
        return Ok((
            DownloadOutcome::Cancelled { written: 0 },
            LeaseReport::new(0, 0, 0, 0, Vec::new(), total_size),
        ));
    }
    if pause_token.is_cancelled() {
        return Ok((
            DownloadOutcome::Paused { written: 0 },
            LeaseReport::new(0, 0, 0, 0, Vec::new(), total_size),
        ));
    }

    // Check if file already exists and has content (resume case)
    let file_exists = part_path.exists();
    let existing_len = if file_exists {
        std::fs::metadata(part_path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    // If resuming with completed ranges, don't truncate; ensure file is at least total_size
    if !completed_ranges.is_empty() || existing_len > 0 {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false) // resume keeps the bytes already on disk
            .open(part_path)
            .map_err(|e| CoreError::io_with_context(e, "open part file for parallel resume"))?;
        if existing_len < total_size {
            file.set_len(total_size).map_err(|e| {
                CoreError::io_with_context(e, "set_len part file for parallel resume")
            })?;
        }
    } else {
        // Fresh download: preallocate
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(part_path)
            .map_err(|e| CoreError::io_with_context(e, "create part file for parallel transfer"))?;
        file.set_len(total_size).map_err(|e| {
            CoreError::io_with_context(e, "set_len part file for parallel transfer")
        })?;
    }

    if total_size == 0 {
        let table = LeaseTable::new(0, chunk_size);
        let report = table.report();
        return Ok((DownloadOutcome::Completed { written: 0 }, report));
    }

    // Open shared file handle allowing concurrent writes
    let mut open_opts = std::fs::OpenOptions::new();
    open_opts.read(true).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        open_opts.share_mode(1 | 2); // FILE_SHARE_READ | FILE_SHARE_WRITE
    }
    let shared_file = Arc::new(
        open_opts
            .open(part_path)
            .map_err(|e| CoreError::io_with_context(e, "open shared part file"))?,
    );

    // Create LeaseTable and mark completed ranges from checkpoint
    let mut table = LeaseTable::new(total_size, chunk_size);
    if !completed_ranges.is_empty() {
        // Convert (start, end_exclusive) to (start, end_inclusive) for LeaseTable
        let inclusive_ranges: Vec<(u64, u64)> = completed_ranges
            .iter()
            .map(|&(s, e)| (s, e.saturating_sub(1)))
            .filter(|&(s, e)| s <= e)
            .collect();
        table.mark_completed_ranges(&inclusive_ranges);
    }
    let table = Arc::new(Mutex::new(table));
    let total_chunks = { table.lock().await.total_chunks() };
    let num_workers = max_connections.min(total_chunks).max(1);

    // Live per-connection snapshot: one slot per worker, read on demand through
    // `connection_snapshot` on the same 500 ms cadence as `Progress`. The
    // registration withdraws the snapshot on every exit path of this transfer.
    let connections = Arc::new(ConnectionRegistry::new(num_workers));
    let _snapshot = SnapshotRegistration::new(task_id, Arc::clone(&connections));

    let total_downloaded = Arc::new(AtomicU64::new(0));
    let fatal_error = Arc::new(Mutex::new(None::<CoreError>));
    let throttled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let range_unsupported = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let start_time = Instant::now();
    // One shared bucket so the per-task cap covers all connections together,
    // not each connection separately. The limit cell is read live so the user
    // can change the cap of a running task.
    let speed_limiter: Arc<Mutex<Option<TokenBucket>>> = Arc::new(Mutex::new(None));

    let mut handles = Vec::with_capacity(num_workers);

    // Clone store for monitor task and final checkpoint, then shadow original to prevent capture
    let store_for_monitor = store.clone();
    let store_for_final = store.clone();
    let _store = (); // Shadow original store to prevent capture in worker closures

    // Spawn monitor task first (captures store_for_monitor)
    let total_dl_clone = Arc::clone(&total_downloaded);
    let event_sender_clone = event_sender.clone();
    let cancel_token_clone = cancel_token.clone();
    let pause_token_clone = pause_token.clone();
    let table_clone = Arc::clone(&table);

    let monitor_handle = tokio::spawn(async move {
        let store_monitor = store_for_monitor.clone();
        let mut interval = tokio::time::interval(Duration::from_millis(500)); // Save checkpoint every 500ms
        let mut last_emitted_bytes = 0u64;
        loop {
            interval.tick().await;
            if cancel_token_clone.is_cancelled() || pause_token_clone.is_cancelled() {
                break;
            }
            let current = total_dl_clone.load(Ordering::Relaxed).min(total_size);
            // Only emit Progress if monotonically non-decreasing to prevent backwards jumps
            if current >= last_emitted_bytes {
                let elapsed = start_time.elapsed().as_secs_f64();
                let speed = if elapsed > 0.0 {
                    (current as f64 / elapsed) as u64
                } else {
                    0
                };
                let _ = event_sender_clone
                    .emit(
                        task_id,
                        EventKind::Progress,
                        EventPayload::Progress {
                            downloaded_bytes: current,
                            total_bytes: Some(total_size),
                            speed_bps: speed,
                        },
                    )
                    .await;
                last_emitted_bytes = current;
            }

            // Save checkpoint periodically
            let ranges = {
                let tbl = table_clone.lock().await;
                tbl.completed_ranges().to_vec()
            };
            if !ranges.is_empty() {
                let completed_ranges: Vec<(u64, u64)> =
                    ranges.iter().map(|&(s, e)| (s, e + 1)).collect();
                let written: u64 = completed_ranges.iter().map(|&(s, e)| e - s).sum();
                let checkpoint =
                    Checkpoint::new(task_id, written, Some(total_size), completed_ranges);
                let _ = store_monitor.save_checkpoint(&checkpoint);
            }

            if current >= total_size {
                break;
            }
        }
    });

    // Spawn workers (store not captured here)
    for worker_id in 0..num_workers {
        let client = client.clone();
        let url = url.clone();
        let shared_file = Arc::clone(&shared_file);
        let total_downloaded = Arc::clone(&total_downloaded);
        let table = Arc::clone(&table);
        let connections = Arc::clone(&connections);
        let fatal_error = Arc::clone(&fatal_error);
        let throttled = Arc::clone(&throttled);
        let range_unsupported = Arc::clone(&range_unsupported);
        let speed_limiter = Arc::clone(&speed_limiter);
        let speed_limit = speed_limit.clone();
        let cancel_token = cancel_token.clone();
        let pause_token = pause_token.clone();
        // `lease_ttl` is Copy and captured by the worker task below through the
        // outer scope, so no explicit clone/rebind is needed here.

        let handle = tokio::spawn(async move {
            let mut consecutive_failures = 0;

            loop {
                if cancel_token.is_cancelled() || pause_token.is_cancelled() {
                    break;
                }
                if range_unsupported.load(Ordering::Relaxed)
                    || throttled.load(Ordering::Relaxed)
                    || fatal_error.lock().await.is_some()
                {
                    break;
                }

                // 1. Claim next lease
                let lease_opt = {
                    let mut tbl = table.lock().await;
                    if tbl.is_complete() {
                        break;
                    }
                    tbl.claim_next(worker_id, Instant::now(), lease_ttl)
                };

                let lease = match lease_opt {
                    Some(l) => l,
                    None => {
                        let is_done = { table.lock().await.is_complete() };
                        if is_done {
                            break;
                        }
                        tokio::select! {
                            _ = cancel_token.cancelled() => break,
                            _ = pause_token.cancelled() => break,
                            _ = tokio::time::sleep(Duration::from_millis(25)) => continue,
                        }
                    }
                };

                // Publish the byte range this connection now owns.
                connections.begin(worker_id, lease.start, lease.end_inclusive + 1);

                // 2. Fetch Range with retry on transient failures
                let range_header_val =
                    sayl_http::range::format_byte_range(lease.start, Some(lease.end_inclusive));
                let built_req = match client
                    .get(url.clone())
                    .header(reqwest::header::ACCEPT_ENCODING, "identity")
                    .header(reqwest::header::RANGE, range_header_val)
                    .build()
                {
                    Ok(r) => r,
                    Err(e) => {
                        let mut tbl = table.lock().await;
                        tbl.release(lease);
                        let err = CoreError::http(None, e.to_string(), None);
                        let mut err_guard = fatal_error.lock().await;
                        if err_guard.is_none() {
                            *err_guard = Some(err);
                        }
                        return Ok(());
                    }
                };

                let retry_policy = sayl_http::retry::RetryPolicy::default();
                let mut req_attempts = 0u32;
                let res_opt: Option<reqwest::Response> = loop {
                    req_attempts += 1;
                    let exec_req = match built_req.try_clone() {
                        Some(r) => r,
                        None => break None,
                    };
                    match client.execute(exec_req).await {
                        Ok(r) => {
                            let status = r.status();
                            let code = status.as_u16();
                            if code == 429 || code == 503 {
                                throttled.store(true, std::sync::atomic::Ordering::Relaxed);
                                let mut tbl = table.lock().await;
                                tbl.release(lease);
                                let retry_after_secs = r
                                    .headers()
                                    .get(reqwest::header::RETRY_AFTER)
                                    .and_then(|v| v.to_str().ok())
                                    .and_then(|s| s.parse::<u64>().ok());
                                let status_text = if code == 429 {
                                    "HTTP 429 Too Many Requests"
                                } else {
                                    "HTTP 503 Service Unavailable"
                                };
                                let err = CoreError::http(
                                    Some(code),
                                    status_text.to_string(),
                                    retry_after_secs,
                                );
                                let mut err_guard = fatal_error.lock().await;
                                if err_guard.is_none() {
                                    *err_guard = Some(err);
                                }
                                tracing::warn!(
                                    "worker {worker_id}: received {code}, signaling fallback to sequential"
                                );
                                break None;
                            }
                            if status != reqwest::StatusCode::PARTIAL_CONTENT
                                && status != reqwest::StatusCode::OK
                            {
                                if sayl_http::retry::is_retryable_status(code)
                                    && retry_policy.may_retry(req_attempts)
                                    && !cancel_token.is_cancelled()
                                {
                                    let retry_after_secs = r
                                        .headers()
                                        .get(reqwest::header::RETRY_AFTER)
                                        .and_then(|v| v.to_str().ok())
                                        .and_then(|s| s.parse::<u64>().ok());
                                    let delay = retry_policy.delay_for_attempt(
                                        req_attempts.saturating_sub(1),
                                        retry_after_secs.map(Duration::from_secs),
                                    );
                                    tracing::warn!(
                                        "worker {worker_id}: retryable status {code} (attempt {req_attempts}), waiting {delay:?}"
                                    );
                                    tokio::time::sleep(delay).await;
                                    continue;
                                }
                                let mut tbl = table.lock().await;
                                tbl.release(lease);
                                let retry_after_secs = r
                                    .headers()
                                    .get(reqwest::header::RETRY_AFTER)
                                    .and_then(|v| v.to_str().ok())
                                    .and_then(|s| s.parse::<u64>().ok());
                                let err = CoreError::http(
                                    Some(code),
                                    format!(
                                        "worker {worker_id} received unexpected status: {status}"
                                    ),
                                    retry_after_secs,
                                );
                                let mut err_guard = fatal_error.lock().await;
                                if err_guard.is_none() {
                                    *err_guard = Some(err);
                                }
                                return Ok(());
                            }
                            // 200 for a Range request means the server ignores Range
                            if status == reqwest::StatusCode::OK {
                                range_unsupported.store(true, std::sync::atomic::Ordering::Relaxed);
                                let mut tbl = table.lock().await;
                                tbl.release(lease);
                                tracing::warn!(
                                    "worker {worker_id}: server returned 200 for Range request, marking range_unsupported"
                                );
                                break None;
                            }
                            break Some(r);
                        }
                        Err(err) => {
                            if retry_policy.may_retry(req_attempts)
                                && !cancel_token.is_cancelled()
                                && !pause_token.is_cancelled()
                            {
                                let delay = retry_policy
                                    .delay_for_attempt(req_attempts.saturating_sub(1), None);
                                tracing::warn!(
                                    "worker {worker_id}: transient error (attempt {req_attempts}), waiting {delay:?}: {err}"
                                );
                                tokio::time::sleep(delay).await;
                                continue;
                            }
                            if cancel_token.is_cancelled() || pause_token.is_cancelled() {
                                let mut tbl = table.lock().await;
                                tbl.release(lease);
                                break None;
                            }
                            let mut tbl = table.lock().await;
                            tbl.release(lease);
                            consecutive_failures += 1;
                            if consecutive_failures >= 3 {
                                let core_err = CoreError::http(
                                    None,
                                    format!("worker {worker_id} request error: {err}"),
                                    None,
                                );
                                let mut err_guard = fatal_error.lock().await;
                                if err_guard.is_none() {
                                    *err_guard = Some(core_err);
                                }
                                break None;
                            }
                            tokio::time::sleep(Duration::from_millis(30)).await;
                            continue;
                        }
                    }
                };

                // If we broke out without a response, stop this worker cleanly
                let res = match res_opt {
                    Some(r) => r,
                    None => break,
                };

                // 3. Stream bytes and write at exact offset
                let mut stream = res.bytes_stream();
                let mut worker_offset = lease.start;
                let expected_end = lease.end_inclusive + 1;
                let mut stream_err = None;

                loop {
                    if cancel_token.is_cancelled() || pause_token.is_cancelled() {
                        let mut tbl = table.lock().await;
                        tbl.release(lease);
                        break;
                    }

                    tokio::select! {
                        _ = cancel_token.cancelled() => {
                            let mut tbl = table.lock().await;
                            tbl.release(lease);
                            break;
                        }
                        _ = pause_token.cancelled() => {
                            let mut tbl = table.lock().await;
                            tbl.release(lease);
                            break;
                        }
                        item = tokio::time::timeout(crate::config::DEFAULT_STALL_TIMEOUT, stream.next()) => {
                            match item {
                                Ok(Some(Ok(chunk_bytes))) => {
                                    if !chunk_bytes.is_empty() {
                                        // Shared live per-task throttle across all workers.
                                        // 0 or absent in the cell means unlimited.
                                        {
                                            let limit_bps = speed_limit
                                                .as_ref()
                                                .map(|cell| cell.load(Ordering::Relaxed))
                                                .unwrap_or(0);
                                            let mut limiter = speed_limiter.lock().await;
                                            if limit_bps > 0 {
                                                let bucket = limiter.get_or_insert_with(|| {
                                                    TokenBucket::new(limit_bps)
                                                });
                                                bucket.set_rate(limit_bps);
                                                let wait = bucket.take(
                                                    chunk_bytes.len() as u64,
                                                    Instant::now(),
                                                );
                                                if wait > Duration::ZERO {
                                                    tokio::time::sleep(wait).await;
                                                }
                                            } else {
                                                *limiter = None;
                                            }
                                        }
                                        if worker_offset + (chunk_bytes.len() as u64) > expected_end {
                                            let mut tbl = table.lock().await;
                                            tbl.release(lease);
                                            let err = CoreError::validation(
                                                "range_lease",
                                                format!(
                                                    "worker {worker_id} received {} bytes exceeding range end {}",
                                                    worker_offset + chunk_bytes.len() as u64,
                                                    expected_end
                                                ),
                                            );
                                            let mut err_guard = fatal_error.lock().await;
                                            if err_guard.is_none() {
                                                *err_guard = Some(err);
                                            }
                                            return Ok(());
                                        }

                                        if let Err(e) = crate::io_util::write_at(
                                            &shared_file,
                                            &chunk_bytes,
                                            worker_offset,
                                        ) {
                                            let mut tbl = table.lock().await;
                                            tbl.release(lease);
                                            let err = CoreError::io_with_context(
                                                e,
                                                format!("worker {worker_id} write_at"),
                                            );
                                            let mut err_guard = fatal_error.lock().await;
                                            if err_guard.is_none() {
                                                *err_guard = Some(err);
                                            }
                                            return Ok(());
                                        }

                                        worker_offset += chunk_bytes.len() as u64;
                                        total_downloaded
                                            .fetch_add(chunk_bytes.len() as u64, Ordering::Relaxed);
                                        connections.advance(worker_id, chunk_bytes.len() as u64);
                                    }
                                }
                                Ok(Some(Err(err))) => {
                                    stream_err = Some(CoreError::http(
                                        None,
                                        format!("worker {worker_id} stream error: {err}"),
                                        None,
                                    ));
                                    break;
                                }
                                Ok(None) => {
                                    break;
                                }
                                Err(_) => {
                                    stream_err = Some(CoreError::timeout(format!(
                                        "worker {worker_id} stalled: no bytes received within timeout"
                                    )));
                                    break;
                                }
                            }
                        }
                    }
                }

                if let Some(err) = stream_err {
                    let mut tbl = table.lock().await;
                    tbl.release(lease);
                    consecutive_failures += 1;
                    if consecutive_failures >= 3 {
                        let mut err_guard = fatal_error.lock().await;
                        if err_guard.is_none() {
                            *err_guard = Some(err);
                        }
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(30)).await;
                    continue;
                }

                // Check cancellation
                if cancel_token.is_cancelled() || pause_token.is_cancelled() {
                    let mut tbl = table.lock().await;
                    tbl.release(lease);
                    break;
                }

                // Verify full range was written
                if worker_offset != expected_end {
                    let mut tbl = table.lock().await;
                    tbl.release(lease);
                    consecutive_failures += 1;
                    if consecutive_failures >= 3 {
                        let err = CoreError::validation(
                            "range",
                            format!(
                                "worker {worker_id} stream truncated: wrote {worker_offset} of expected {expected_end}"
                            ),
                        );
                        let mut err_guard = fatal_error.lock().await;
                        if err_guard.is_none() {
                            *err_guard = Some(err);
                        }
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(30)).await;
                    continue;
                }

                // 4. Complete
                {
                    let mut tbl = table.lock().await;
                    let completed = tbl.complete(lease);
                    if completed {
                        connections.finish(worker_id);
                        consecutive_failures = 0;
                    }
                }
            }

            Ok::<(), CoreError>(())
        });

        handles.push(handle);
    }

    // Wait for all workers to complete
    for handle in handles {
        let _ = handle.await;
    }

    monitor_handle.abort();

    // Check cancellation
    if cancel_token.is_cancelled() {
        let mut tbl = table.lock().await;
        tbl.release_all_active();
        let report = tbl.report();
        let _ = shared_file.sync_all();
        drop(shared_file);

        let completed_ranges: Vec<(u64, u64)> = report
            .completed_ranges
            .iter()
            .map(|&(s, e)| (s, e + 1))
            .collect();
        let current_written: u64 = completed_ranges.iter().map(|&(s, e)| e - s).sum();

        let checkpoint =
            Checkpoint::new(task_id, current_written, Some(total_size), completed_ranges);
        let _ = store_for_final.save_checkpoint(&checkpoint);
        return Ok((
            DownloadOutcome::Cancelled {
                written: current_written,
            },
            report,
        ));
    }

    // Check pause
    if pause_token.is_cancelled() {
        let mut tbl = table.lock().await;
        tbl.release_all_active();
        let report = tbl.report();
        let _ = shared_file.sync_all();
        drop(shared_file);

        let completed_ranges: Vec<(u64, u64)> = report
            .completed_ranges
            .iter()
            .map(|&(s, e)| (s, e + 1))
            .collect();
        let current_written: u64 = completed_ranges.iter().map(|&(s, e)| e - s).sum();

        let checkpoint =
            Checkpoint::new(task_id, current_written, Some(total_size), completed_ranges);
        let _ = store_for_final.save_checkpoint(&checkpoint);
        let _ = event_sender
            .emit(
                task_id,
                EventKind::Checkpoint,
                EventPayload::Checkpoint {
                    durable_bytes: current_written,
                    total_bytes: Some(total_size),
                },
            )
            .await;
        return Ok((
            DownloadOutcome::Paused {
                written: current_written,
            },
            report,
        ));
    }

    let report = { table.lock().await.report() };

    // Check if table successfully finished all chunks
    if report.chunks_completed == report.chunks_total && report.chunks_total > 0 {
        shared_file
            .sync_all()
            .map_err(|e| CoreError::io_with_context(e, "sync shared part file"))?;
        drop(shared_file);

        // Verify full coverage with no overlap
        report.assert_full_coverage(total_size);

        let completed_ranges: Vec<(u64, u64)> = report
            .completed_ranges
            .iter()
            .map(|&(s, e)| (s, e + 1))
            .collect();

        let checkpoint = Checkpoint::new(task_id, total_size, Some(total_size), completed_ranges);
        let _ = store_for_final.save_checkpoint(&checkpoint);
        let _ = event_sender
            .emit(
                task_id,
                EventKind::Checkpoint,
                EventPayload::Checkpoint {
                    durable_bytes: total_size,
                    total_bytes: Some(total_size),
                },
            )
            .await;

        return Ok((
            DownloadOutcome::Completed {
                written: total_size,
            },
            report,
        ));
    }

    // Did not complete all chunks
    let _ = shared_file.sync_all();
    drop(shared_file);

    // Save partial progress from completed chunks so fallback can resume from it
    let completed_ranges: Vec<(u64, u64)> = report
        .completed_ranges
        .iter()
        .map(|&(s, e)| (s, e + 1))
        .collect();
    let current_written: u64 = completed_ranges.iter().map(|&(s, e)| e - s).sum();
    if !completed_ranges.is_empty() {
        let checkpoint =
            Checkpoint::new(task_id, current_written, Some(total_size), completed_ranges);
        let _ = store_for_final.save_checkpoint(&checkpoint);
    }

    // If any worker detected that the server ignores Range requests (200 for Range),
    // return a range_unsupported error so the engine can fall back to sequential.
    if range_unsupported.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(CoreError::http(
            Some(200),
            "server returned 200 for a Range request; parallel transfer not supported".to_string(),
            None,
        ));
    }

    if throttled.load(std::sync::atomic::Ordering::Relaxed) {
        let err_opt = { fatal_error.lock().await.take() };
        if let Some(err) = err_opt {
            return Err(err);
        }
        return Err(CoreError::http(
            Some(429),
            "HTTP 429 Too Many Requests".to_string(),
            None,
        ));
    }

    let err_opt = { fatal_error.lock().await.take() };
    if let Some(err) = err_opt {
        return Err(err);
    }

    Err(CoreError::validation(
        "content_length",
        format!(
            "parallel transfer incomplete: completed {} of {} chunks",
            report.chunks_completed, report.chunks_total
        ),
    ))
}

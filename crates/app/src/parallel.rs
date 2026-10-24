//! Bounded parallel multi-connection download implementation using dynamic lease scheduling.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use sayl_core::error::CoreError;
use sayl_core::event::{EventKind, EventPayload, EventSender};
use sayl_core::id::TaskId;
use sayl_storage::{Checkpoint, Store};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::lease::{LeaseReport, LeaseTable, DEFAULT_CHUNK_SIZE, DEFAULT_LEASE_TTL};
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

/// Executes a bounded parallel multi-connection download to a `.part` file.
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
    store: &Store,
) -> Result<DownloadOutcome, CoreError> {
    let options = ParallelOptions {
        chunk_size,
        lease_ttl: DEFAULT_LEASE_TTL,
    };
    let (outcome, _) = run_parallel_download_with_options(
        client,
        url,
        part_path,
        task_id,
        total_size,
        max_connections,
        options,
        cancel_token,
        pause_token,
        event_sender,
        store,
    )
    .await?;
    Ok(outcome)
}

/// Executes a parallel download and returns both the [`DownloadOutcome`] and the resulting [`LeaseReport`].
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
    store: &Store,
) -> Result<(DownloadOutcome, LeaseReport), CoreError> {
    let options = ParallelOptions {
        chunk_size,
        lease_ttl: DEFAULT_LEASE_TTL,
    };
    run_parallel_download_with_options(
        client,
        url,
        part_path,
        task_id,
        total_size,
        max_connections,
        options,
        cancel_token,
        pause_token,
        event_sender,
        store,
    )
    .await
}

/// Executes a parallel download with fully configurable [`ParallelOptions`].
pub async fn run_parallel_download_with_options(
    client: &reqwest::Client,
    url: &Url,
    part_path: &Path,
    task_id: TaskId,
    total_size: u64,
    max_connections: usize,
    options: ParallelOptions,
    cancel_token: CancellationToken,
    pause_token: CancellationToken,
    event_sender: &EventSender,
    store: &Store,
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

    // Preallocate destination file size on disk
    {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(part_path)
            .map_err(|e| CoreError::io_with_context(e, "create part file for parallel transfer"))?;
        file.set_len(total_size)
            .map_err(|e| CoreError::io_with_context(e, "set_len part file for parallel transfer"))?;
    }

    if total_size == 0 {
        let table = LeaseTable::new(0, options.chunk_size);
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

    let table = Arc::new(Mutex::new(LeaseTable::new(total_size, options.chunk_size)));
    let total_chunks = { table.lock().await.total_chunks() };
    let num_workers = max_connections.min(total_chunks).max(1);

    let total_downloaded = Arc::new(AtomicU64::new(0));
    let first_error = Arc::new(Mutex::new(None::<CoreError>));
    let start_time = Instant::now();

    let mut handles = Vec::with_capacity(num_workers);

    for worker_id in 0..num_workers {
        let client = client.clone();
        let url = url.clone();
        let shared_file = Arc::clone(&shared_file);
        let total_downloaded = Arc::clone(&total_downloaded);
        let table = Arc::clone(&table);
        let first_error = Arc::clone(&first_error);
        let cancel_token = cancel_token.clone();
        let pause_token = pause_token.clone();
        let lease_ttl = options.lease_ttl;

        let handle = tokio::spawn(async move {
            let mut consecutive_failures = 0;

            loop {
                if cancel_token.is_cancelled() || pause_token.is_cancelled() {
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

                // 2. Fetch Range
                let range_header_val = sayl_http::range::format_byte_range(
                    lease.start,
                    Some(lease.end_inclusive),
                );
                let req = match client
                    .get(url.clone())
                    .header(reqwest::header::ACCEPT_ENCODING, "identity")
                    .header(reqwest::header::RANGE, range_header_val)
                    .build()
                {
                    Ok(r) => r,
                    Err(e) => {
                        let mut tbl = table.lock().await;
                        tbl.release(&lease);
                        let err = CoreError::http(None, e.to_string(), None);
                        let mut err_guard = first_error.lock().await;
                        if err_guard.is_none() {
                            *err_guard = Some(err);
                        }
                        return Ok(());
                    }
                };

                let res = match client.execute(req).await {
                    Ok(r) => r,
                    Err(err) => {
                        let mut tbl = table.lock().await;
                        tbl.release(&lease);

                        if cancel_token.is_cancelled() || pause_token.is_cancelled() {
                            break;
                        }

                        let core_err = CoreError::http(
                            None,
                            format!("worker {worker_id} request error: {err}"),
                            None,
                        );
                        let mut err_guard = first_error.lock().await;
                        if err_guard.is_none() {
                            *err_guard = Some(core_err);
                        }

                        consecutive_failures += 1;
                        if consecutive_failures >= 3 {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(30)).await;
                        continue;
                    }
                };

                let status = res.status();
                if status != reqwest::StatusCode::PARTIAL_CONTENT
                    && status != reqwest::StatusCode::OK
                {
                    let mut tbl = table.lock().await;
                    tbl.release(&lease);
                    let err = CoreError::http(
                        Some(status.as_u16()),
                        format!("worker {worker_id} received unexpected status: {status}"),
                        None,
                    );
                    let mut err_guard = first_error.lock().await;
                    if err_guard.is_none() {
                        *err_guard = Some(err);
                    }
                    return Ok(());
                }

                // 3. Stream bytes and write at exact offset
                let mut stream = res.bytes_stream();
                let mut worker_offset = lease.start;
                let expected_end = lease.end_inclusive + 1;
                let mut stream_err = None;

                loop {
                    if cancel_token.is_cancelled() || pause_token.is_cancelled() {
                        let mut tbl = table.lock().await;
                        tbl.release(&lease);
                        break;
                    }

                    tokio::select! {
                        _ = cancel_token.cancelled() => {
                            let mut tbl = table.lock().await;
                            tbl.release(&lease);
                            break;
                        }
                        _ = pause_token.cancelled() => {
                            let mut tbl = table.lock().await;
                            tbl.release(&lease);
                            break;
                        }
                        item = stream.next() => {
                            match item {
                                Some(Ok(chunk_bytes)) => {
                                    if !chunk_bytes.is_empty() {
                                        if worker_offset + (chunk_bytes.len() as u64) > expected_end {
                                            let mut tbl = table.lock().await;
                                            tbl.release(&lease);
                                            let err = CoreError::validation(
                                                "range_lease",
                                                format!(
                                                    "worker {worker_id} received {} bytes exceeding range end {}",
                                                    worker_offset + chunk_bytes.len() as u64,
                                                    expected_end
                                                ),
                                            );
                                            let mut err_guard = first_error.lock().await;
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
                                            tbl.release(&lease);
                                            let err = CoreError::io_with_context(
                                                e,
                                                format!("worker {worker_id} write_at"),
                                            );
                                            let mut err_guard = first_error.lock().await;
                                            if err_guard.is_none() {
                                                *err_guard = Some(err);
                                            }
                                            return Ok(());
                                        }

                                        worker_offset += chunk_bytes.len() as u64;
                                        total_downloaded
                                            .fetch_add(chunk_bytes.len() as u64, Ordering::Relaxed);
                                    }
                                }
                                Some(Err(err)) => {
                                    stream_err = Some(CoreError::http(
                                        None,
                                        format!("worker {worker_id} stream error: {err}"),
                                        None,
                                    ));
                                    break;
                                }
                                None => {
                                    break;
                                }
                            }
                        }
                    }
                }

                if let Some(err) = stream_err {
                    let mut tbl = table.lock().await;
                    tbl.release(&lease);
                    let mut err_guard = first_error.lock().await;
                    if err_guard.is_none() {
                        *err_guard = Some(err);
                    }
                    consecutive_failures += 1;
                    if consecutive_failures >= 3 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(30)).await;
                    continue;
                }

                // Check cancellation
                if cancel_token.is_cancelled() || pause_token.is_cancelled() {
                    let mut tbl = table.lock().await;
                    tbl.release(&lease);
                    break;
                }

                // Verify full range was written
                if worker_offset != expected_end {
                    let mut tbl = table.lock().await;
                    tbl.release(&lease);
                    let err = CoreError::validation(
                        "range",
                        format!(
                            "worker {worker_id} stream truncated: wrote {worker_offset} of expected {expected_end}"
                        ),
                    );
                    let mut err_guard = first_error.lock().await;
                    if err_guard.is_none() {
                        *err_guard = Some(err);
                    }
                    consecutive_failures += 1;
                    if consecutive_failures >= 3 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(30)).await;
                    continue;
                }

                // 4. Complete
                {
                    let mut tbl = table.lock().await;
                    let completed = tbl.complete(&lease);
                    if completed {
                        consecutive_failures = 0;
                    }
                }
            }

            Ok::<(), CoreError>(())
        });

        handles.push(handle);
    }

    // Periodic progress reporter task while workers run
    let total_dl_clone = Arc::clone(&total_downloaded);
    let event_sender_clone = event_sender.clone();
    let cancel_token_clone = cancel_token.clone();
    let pause_token_clone = pause_token.clone();

    let monitor_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(50));
        loop {
            interval.tick().await;
            if cancel_token_clone.is_cancelled() || pause_token_clone.is_cancelled() {
                break;
            }
            let current = total_dl_clone.load(Ordering::Relaxed).min(total_size);
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
            if current >= total_size {
                break;
            }
        }
    });

    // Wait for all workers to complete
    for handle in handles {
        let _ = handle.await;
    }

    let _ = monitor_handle.abort();

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
        let _ = store.save_checkpoint(&checkpoint);
        return Ok((DownloadOutcome::Cancelled { written: current_written }, report));
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
        let _ = store.save_checkpoint(&checkpoint);
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
        return Ok((DownloadOutcome::Paused { written: current_written }, report));
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

        let checkpoint =
            Checkpoint::new(task_id, total_size, Some(total_size), completed_ranges);
        let _ = store.save_checkpoint(&checkpoint);
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

    let err_opt = { first_error.lock().await.take() };
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

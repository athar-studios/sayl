//! Bounded parallel multi-connection download implementation.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use futures_util::StreamExt;
use sayl_core::error::CoreError;
use sayl_core::event::{EventKind, EventPayload, EventSender};
use sayl_core::id::TaskId;
use sayl_storage::{Checkpoint, Store};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::sequential::DownloadOutcome;

/// Executes a bounded parallel multi-connection download to a `.part` file.
pub async fn run_parallel_download(
    client: &reqwest::Client,
    url: &Url,
    part_path: &Path,
    task_id: TaskId,
    total_size: u64,
    max_connections: usize,
    cancel_token: CancellationToken,
    pause_token: CancellationToken,
    event_sender: &EventSender,
    store: &Store,
) -> Result<DownloadOutcome, CoreError> {
    if cancel_token.is_cancelled() {
        return Ok(DownloadOutcome::Cancelled { written: 0 });
    }
    if pause_token.is_cancelled() {
        return Ok(DownloadOutcome::Paused { written: 0 });
    }

    let num_connections = max_connections.min(total_size as usize).max(1);

    // Split [0, total_size) into strictly disjoint non-overlapping ranges
    let chunk_len = total_size / (num_connections as u64);
    let mut ranges: Vec<(u64, u64)> = Vec::with_capacity(num_connections);
    for i in 0..num_connections {
        let start = (i as u64) * chunk_len;
        let end = if i == num_connections - 1 {
            total_size
        } else {
            (i as u64 + 1) * chunk_len
        };
        ranges.push((start, end));
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

    let total_downloaded = Arc::new(AtomicU64::new(0));
    let start_time = Instant::now();

    // Spawn workers for each range lease
    let mut handles = Vec::with_capacity(num_connections);

    for (worker_idx, &(range_start, range_end)) in ranges.iter().enumerate() {
        let client = client.clone();
        let url = url.clone();
        let shared_file = Arc::clone(&shared_file);
        let total_downloaded = Arc::clone(&total_downloaded);
        let cancel_token = cancel_token.clone();
        let pause_token = pause_token.clone();

        let handle = tokio::spawn(async move {
            if cancel_token.is_cancelled() || pause_token.is_cancelled() {
                return Ok::<u64, CoreError>(range_start);
            }

            // Subrequest with range header
            let range_header_val = sayl_http::range::format_byte_range(range_start, Some(range_end - 1));
            let req = client
                .get(url)
                .header(reqwest::header::ACCEPT_ENCODING, "identity")
                .header(reqwest::header::RANGE, range_header_val)
                .build()
                .map_err(|e| CoreError::http(None, e.to_string(), None))?;

            let res = client
                .execute(req)
                .await
                .map_err(|e| CoreError::http(None, e.to_string(), None))?;

            let status = res.status();
            if status != reqwest::StatusCode::PARTIAL_CONTENT && status != reqwest::StatusCode::OK {
                return Err(CoreError::http(
                    Some(status.as_u16()),
                    format!("worker {worker_idx} received unexpected status: {status}"),
                    None,
                ));
            }

            let mut stream = res.bytes_stream();
            let mut worker_offset = range_start;

            loop {
                if cancel_token.is_cancelled() || pause_token.is_cancelled() {
                    return Ok(worker_offset);
                }

                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        return Ok(worker_offset);
                    }
                    _ = pause_token.cancelled() => {
                        return Ok(worker_offset);
                    }
                    item = stream.next() => {
                        match item {
                            Some(Ok(chunk)) => {
                                if !chunk.is_empty() {
                                    // Enforce range lease: worker must never write beyond range_end
                                    if worker_offset + (chunk.len() as u64) > range_end {
                                        return Err(CoreError::validation(
                                            "range_lease",
                                            format!(
                                                "worker {worker_idx} received {} bytes exceeding range end {}",
                                                worker_offset + (chunk.len() as u64),
                                                range_end
                                            ),
                                        ));
                                    }

                                    // Positioned write into shared file
                                    crate::io_util::write_at(&shared_file, &chunk, worker_offset)
                                        .map_err(|e| CoreError::io_with_context(e, format!("worker {worker_idx} write_at")))?;

                                    worker_offset += chunk.len() as u64;
                                    total_downloaded.fetch_add(chunk.len() as u64, Ordering::Relaxed);
                                }
                            }
                            Some(Err(err)) => {
                                return Err(CoreError::http(
                                    None,
                                    format!("worker {worker_idx} stream error: {err}"),
                                    None,
                                ));
                            }
                            None => {
                                break;
                            }
                        }
                    }
                }
            }

            // Verify full range was written
            if worker_offset != range_end {
                return Err(CoreError::validation(
                    "range",
                    format!(
                        "worker {worker_idx} stream truncated: wrote {worker_offset} of expected {range_end}"
                    ),
                ));
            }

            Ok(worker_offset)
        });

        handles.push(handle);
    }

    // Periodic progress reporter task while workers run
    let total_dl_clone = Arc::clone(&total_downloaded);
    let event_sender_clone = event_sender.clone();
    let cancel_token_clone = cancel_token.clone();
    let pause_token_clone = pause_token.clone();

    let monitor_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(50));
        loop {
            interval.tick().await;
            if cancel_token_clone.is_cancelled() || pause_token_clone.is_cancelled() {
                break;
            }
            let current = total_dl_clone.load(Ordering::Relaxed);
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
    let mut worker_results = Vec::with_capacity(num_connections);
    let mut first_error = None;

    for handle in handles {
        match handle.await {
            Ok(Ok(offset)) => {
                worker_results.push(offset);
            }
            Ok(Err(err)) => {
                if first_error.is_none() {
                    first_error = Some(err);
                }
            }
            Err(join_err) => {
                if first_error.is_none() {
                    first_error = Some(CoreError::internal(format!("worker panicked: {join_err}")));
                }
            }
        }
    }

    let _ = monitor_handle.abort();

    // Check cancellation
    if cancel_token.is_cancelled() {
        let current_written = total_downloaded.load(Ordering::Relaxed);
        let _ = shared_file.sync_all();
        drop(shared_file);

        let mut range_map = Vec::new();
        for (i, &offset) in worker_results.iter().enumerate() {
            if i < ranges.len() {
                range_map.push((ranges[i].0, offset));
            }
        }
        let checkpoint = Checkpoint::new(task_id, current_written, Some(total_size), range_map);
        let _ = store.save_checkpoint(&checkpoint);
        return Ok(DownloadOutcome::Cancelled { written: current_written });
    }

    // Check pause
    if pause_token.is_cancelled() {
        let current_written = total_downloaded.load(Ordering::Relaxed);
        let _ = shared_file.sync_all();
        drop(shared_file);

        let mut range_map = Vec::new();
        for (i, &offset) in worker_results.iter().enumerate() {
            if i < ranges.len() {
                range_map.push((ranges[i].0, offset));
            }
        }
        let checkpoint = Checkpoint::new(task_id, current_written, Some(total_size), range_map);
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
        return Ok(DownloadOutcome::Paused { written: current_written });
    }

    // Check errors
    if let Some(err) = first_error {
        let _ = shared_file.sync_all();
        drop(shared_file);
        return Err(err);
    }

    // All workers finished successfully
    shared_file
        .sync_all()
        .map_err(|e| CoreError::io_with_context(e, "sync shared part file"))?;
    drop(shared_file);

    let current_written = total_downloaded.load(Ordering::Relaxed);

    // Save final checkpoint
    let checkpoint = Checkpoint::new(
        task_id,
        current_written,
        Some(total_size),
        ranges.clone(),
    );
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

    // Verify written count equals expected total size
    if current_written != total_size {
        return Err(CoreError::validation(
            "content_length",
            format!("parallel transfer wrote {current_written} bytes, expected {total_size}"),
        ));
    }

    Ok(DownloadOutcome::Completed { written: current_written })
}

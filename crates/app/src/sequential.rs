//! Sequential streaming download implementation.

use std::path::Path;
use std::time::Instant;

use futures_util::StreamExt;
use sayl_core::error::CoreError;
use sayl_core::event::{EventKind, EventPayload, EventSender};
use sayl_core::id::TaskId;
use sayl_http::resume::ResumeInfo;
use sayl_storage::{Checkpoint, Store};
use tokio_util::sync::CancellationToken;
use url::Url;

/// Outcome of a sequential transfer operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadOutcome {
    Completed { written: u64 },
    Paused { written: u64 },
    Cancelled { written: u64 },
}

/// Executes a sequential streaming download to a `.part` file.
#[allow(clippy::too_many_arguments)]
pub async fn run_sequential_download(
    client: &reqwest::Client,
    url: &Url,
    part_path: &Path,
    task_id: TaskId,
    expected_length: Option<u64>,
    resume_info: Option<&ResumeInfo>,
    resume_from: u64,
    cancel_token: CancellationToken,
    pause_token: CancellationToken,
    event_sender: &EventSender,
    store: &Store,
    stall_timeout: std::time::Duration,
    custom_headers: Option<&[(String, String)]>,
) -> Result<DownloadOutcome, CoreError> {
    if cancel_token.is_cancelled() {
        return Ok(DownloadOutcome::Cancelled {
            written: resume_from,
        });
    }
    if pause_token.is_cancelled() {
        return Ok(DownloadOutcome::Paused {
            written: resume_from,
        });
    }

    let mut req_builder = client.get(url.clone());
    req_builder = req_builder.header(reqwest::header::ACCEPT_ENCODING, "identity");

    if let Some(headers) = custom_headers {
        if let Ok(hdr_map) = sayl_http::headers::build_request_headers(headers) {
            req_builder = req_builder.headers(hdr_map);
        }
    }

    if resume_from > 0 {
        req_builder = req_builder.header(
            reqwest::header::RANGE,
            sayl_http::range::format_byte_range(resume_from, None),
        );
        if let Some(info) = resume_info {
            if let Some(if_range) = sayl_http::resume::if_range_value(info) {
                req_builder = req_builder.header(reqwest::header::IF_RANGE, if_range);
            }
        }
    }

    let built_req = req_builder
        .build()
        .map_err(|e| CoreError::http(None, e.to_string(), None))?;

    // Retry loop for transient failures (429, 503, 408, 425, 5xx) with backoff
    let retry_policy = sayl_http::retry::RetryPolicy::default();
    let mut attempts = 0u32;
    let res = loop {
        attempts += 1;
        let send_result = match tokio::time::timeout(
            stall_timeout,
            client.execute(built_req.try_clone().expect("GET request is cloneable")),
        )
        .await
        {
            Ok(res) => res,
            Err(_) => {
                return Err(CoreError::timeout(
                    "connection stalled: no response received within timeout",
                ));
            }
        };
        match send_result {
            Ok(r) => {
                let status = r.status();
                if status.is_client_error() || status.is_server_error() {
                    let code = status.as_u16();
                    if sayl_http::retry::is_retryable_status(code)
                        && retry_policy.may_retry(attempts)
                        && !cancel_token.is_cancelled()
                    {
                        let retry_after_secs = r
                            .headers()
                            .get(reqwest::header::RETRY_AFTER)
                            .and_then(|v| v.to_str().ok())
                            .and_then(|s| s.parse::<u64>().ok());
                        let retry_after = retry_after_secs.map(std::time::Duration::from_secs);
                        let delay =
                            retry_policy.delay_for_attempt(attempts.saturating_sub(1), retry_after);
                        tracing::warn!(
                            "retryable status {code} (attempt {attempts}/{}), waiting {delay:?}",
                            retry_policy.max_attempts
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    let retry_after_secs = r
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok());
                    return Err(CoreError::http(
                        Some(code),
                        format!("HTTP {status}"),
                        retry_after_secs,
                    ));
                }
                break r;
            }
            Err(e) => {
                if retry_policy.may_retry(attempts) && !cancel_token.is_cancelled() {
                    let delay = retry_policy.delay_for_attempt(attempts.saturating_sub(1), None);
                    tracing::warn!(
                        "transient request error (attempt {attempts}/{}), waiting {delay:?}: {e}",
                        retry_policy.max_attempts
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Err(CoreError::http(None, e.to_string(), None));
            }
        }
    };

    let status = res.status();
    let is_partial = status == reqwest::StatusCode::PARTIAL_CONTENT;
    let mut valid_partial = is_partial;

    if is_partial {
        if let Some(cr_val) = res.headers().get(reqwest::header::CONTENT_RANGE) {
            if let Ok(cr_str) = cr_val.to_str() {
                if let Ok(cr) = sayl_http::range::ContentRange::parse(cr_str) {
                    if let (Some(expected), Some(total)) = (expected_length, cr.total) {
                        if expected != total {
                            return Err(CoreError::validation(
                                "content_range",
                                format!(
                                    "server Content-Range total ({total}) does not match expected ({expected})"
                                ),
                            ));
                        }
                    }
                    if let Some(start) = cr.start {
                        if start != resume_from {
                            valid_partial = false;
                        }
                    }
                }
            }
        }
    }

    let (mut file, mut current_written) = if resume_from > 0 && valid_partial {
        use tokio::io::AsyncSeekExt;
        let mut f = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false) // resume keeps the bytes already on disk
            .open(part_path)
            .await
            .map_err(|e| CoreError::io_with_context(e, "open part file for resume"))?;
        f.seek(std::io::SeekFrom::Start(resume_from))
            .await
            .map_err(|e| CoreError::io_with_context(e, "seek to resume offset"))?;
        (f, resume_from)
    } else {
        let f = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(part_path)
            .await
            .map_err(|e| CoreError::io_with_context(e, "open part file"))?;
        (f, 0)
    };

    let initial_written = current_written;
    let mut stream = res.bytes_stream();
    let mut last_checkpoint_bytes = current_written;
    let start_time = Instant::now();

    loop {
        if cancel_token.is_cancelled() {
            use tokio::io::AsyncWriteExt;
            let _ = file.flush().await;
            let _ = file.sync_all().await;
            drop(file);
            let checkpoint = Checkpoint::new(
                task_id,
                current_written,
                expected_length,
                vec![(0, current_written)],
            );
            let _ = store.save_checkpoint(&checkpoint);
            return Ok(DownloadOutcome::Cancelled {
                written: current_written,
            });
        }
        if pause_token.is_cancelled() {
            use tokio::io::AsyncWriteExt;
            let _ = file.flush().await;
            let _ = file.sync_all().await;
            drop(file);
            let checkpoint = Checkpoint::new(
                task_id,
                current_written,
                expected_length,
                vec![(0, current_written)],
            );
            let _ = store.save_checkpoint(&checkpoint);
            let _ = event_sender
                .emit(
                    task_id,
                    EventKind::Checkpoint,
                    EventPayload::Checkpoint {
                        durable_bytes: current_written,
                        total_bytes: expected_length,
                    },
                )
                .await;
            return Ok(DownloadOutcome::Paused {
                written: current_written,
            });
        }

        tokio::select! {
            _ = cancel_token.cancelled() => {
                use tokio::io::AsyncWriteExt;
                let _ = file.flush().await;
                let _ = file.sync_all().await;
                drop(file);
                let checkpoint = Checkpoint::new(
                    task_id,
                    current_written,
                    expected_length,
                    vec![(0, current_written)],
                );
                let _ = store.save_checkpoint(&checkpoint);
                return Ok(DownloadOutcome::Cancelled { written: current_written });
            }
            _ = pause_token.cancelled() => {
                use tokio::io::AsyncWriteExt;
                let _ = file.flush().await;
                let _ = file.sync_all().await;
                drop(file);
                let checkpoint = Checkpoint::new(
                    task_id,
                    current_written,
                    expected_length,
                    vec![(0, current_written)],
                );
                let _ = store.save_checkpoint(&checkpoint);
                let _ = event_sender.emit(
                    task_id,
                    EventKind::Checkpoint,
                    EventPayload::Checkpoint {
                        durable_bytes: current_written,
                        total_bytes: expected_length,
                    },
                ).await;
                return Ok(DownloadOutcome::Paused { written: current_written });
            }
            item = tokio::time::timeout(stall_timeout, stream.next()) => {
                match item {
                    Ok(Some(Ok(chunk))) => {
                        if !chunk.is_empty() {
                            use tokio::io::AsyncWriteExt;
                            file.write_all(&chunk)
                                .await
                                .map_err(|e| CoreError::io_with_context(e, "write chunk to part file"))?;
                            current_written += chunk.len() as u64;

                            // Checkpoint every ~1 MiB (1024 * 1024 bytes)
                            if current_written.saturating_sub(last_checkpoint_bytes) >= 1024 * 1024 {
                                let _ = file.flush().await;
                                let _ = file.sync_all().await;
                                let checkpoint = Checkpoint::new(
                                    task_id,
                                    current_written,
                                    expected_length,
                                    vec![(0, current_written)],
                                );
                                let _ = store.save_checkpoint(&checkpoint);
                                let _ = event_sender.emit(
                                    task_id,
                                    EventKind::Checkpoint,
                                    EventPayload::Checkpoint {
                                        durable_bytes: current_written,
                                        total_bytes: expected_length,
                                    },
                                ).await;
                                last_checkpoint_bytes = current_written;
                            }

                            // Emit progress tick
                            let elapsed = start_time.elapsed().as_secs_f64();
                            let speed = if elapsed > 0.0 {
                                ((current_written.saturating_sub(initial_written)) as f64 / elapsed) as u64
                            } else {
                                0
                            };
                            let _ = event_sender.emit(
                                task_id,
                                EventKind::Progress,
                                EventPayload::Progress {
                                    downloaded_bytes: current_written,
                                    total_bytes: expected_length,
                                    speed_bps: speed,
                                },
                            ).await;
                        }
                    }
                    Ok(Some(Err(err))) => {
                        use tokio::io::AsyncWriteExt;
                        let _ = file.flush().await;
                        let _ = file.sync_all().await;
                        drop(file);
                        let checkpoint = Checkpoint::new(
                            task_id,
                            current_written,
                            expected_length,
                            vec![(0, current_written)],
                        );
                        let _ = store.save_checkpoint(&checkpoint);
                        return Err(CoreError::http(None, format!("stream error: {err}"), None));
                    }
                    Ok(None) => {
                        // EOF
                        break;
                    }
                    Err(_) => {
                        use tokio::io::AsyncWriteExt;
                        let _ = file.flush().await;
                        let _ = file.sync_all().await;
                        drop(file);
                        let checkpoint = Checkpoint::new(
                            task_id,
                            current_written,
                            expected_length,
                            vec![(0, current_written)],
                        );
                        let _ = store.save_checkpoint(&checkpoint);
                        return Err(CoreError::timeout(
                            "sequential transfer stalled: no bytes received within timeout",
                        ));
                    }
                }
            }
        }
    }

    use tokio::io::AsyncWriteExt;
    file.flush()
        .await
        .map_err(|e| CoreError::io_with_context(e, "flush part file"))?;
    file.sync_all()
        .await
        .map_err(|e| CoreError::io_with_context(e, "sync part file"))?;
    drop(file);

    // Verify expected length if specified
    if let Some(expected) = expected_length {
        if current_written < expected {
            let checkpoint = Checkpoint::new(
                task_id,
                current_written,
                expected_length,
                vec![(0, current_written)],
            );
            let _ = store.save_checkpoint(&checkpoint);
            return Err(CoreError::validation(
                "content_length",
                format!("body truncated: received {current_written} of {expected} bytes"),
            ));
        } else if current_written > expected {
            return Err(CoreError::validation(
                "content_length",
                format!("received {current_written} bytes exceeding expected {expected} bytes"),
            ));
        }
    }

    Ok(DownloadOutcome::Completed {
        written: current_written,
    })
}

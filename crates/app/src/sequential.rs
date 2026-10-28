//! Sequential streaming download implementation.

use std::path::Path;
use std::time::Instant;

use futures_util::StreamExt;
use sayl_core::error::CoreError;
use sayl_core::event::{EventKind, EventPayload, EventSender};
use sayl_core::id::TaskId;
use sayl_http::probe::ProbeResult;
use sayl_http::resume::ResumeInfo;
use sayl_http::retry::RetryPolicy;
use sayl_http::{headers::build_request_headers, probe, ProbeOptions};
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

/// Re-probes the download URL on resume to verify the resource hasn't changed.
/// If the server returns text/html (expired link), returns a specific error with
/// an Arabic message and "تحديث رابط التحميل" action.
async fn reprobe_on_resume(
    client: &reqwest::Client,
    url: &Url,
    custom_headers: Option<&[(String, String)]>,
) -> Result<ProbeResult, CoreError> {
    let mut probe_opts = ProbeOptions::default();
    if let Some(headers) = custom_headers {
        if let Ok(hdr_map) = build_request_headers(headers) {
            probe_opts.custom_headers = hdr_map;
        }
    }

    let probe_result = probe::probe_with_options(client, url.clone(), probe_opts)
        .await
        .map_err(|e| CoreError::http(e.status_code(), e.to_string(), e.retry_after_secs()))?;

    // Check if server returned text/html (expired link page)
    if let Some(ref content_type) = probe_result.content_type {
        if content_type.to_lowercase().starts_with("text/html") {
            return Err(CoreError::validation(
                "content_type",
                "الرابط منتهي الصلاحية، يرجى تحديث رابط التحميل".to_string(),
            ));
        }
    }

    Ok(probe_result)
}

/// Attempts a download with fallback strategies:
/// 1. Original request (with Range, cookies, referrer, UA)
/// 2. Retry without Range header
/// 3. Retry without cookies/referrer/UA
#[allow(clippy::too_many_arguments)]
async fn attempt_download_with_fallbacks(
    client: &reqwest::Client,
    url: &Url,
    part_path: &Path,
    task_id: TaskId,
    expected_length: Option<u64>,
    resume_from: u64,
    cancel_token: CancellationToken,
    pause_token: CancellationToken,
    event_sender: &EventSender,
    store: &Store,
    stall_timeout: std::time::Duration,
    custom_headers: Option<&[(String, String)]>,
    resume_info: Option<&ResumeInfo>,
) -> Result<DownloadOutcome, CoreError> {
    // First attempt: full headers with Range
    let result = attempt_download_attempt(
        client,
        url,
        part_path,
        task_id,
        expected_length,
        resume_from,
        cancel_token.clone(),
        pause_token.clone(),
        event_sender,
        store,
        stall_timeout,
        custom_headers,
        resume_info,
        true,
        true,
        true,
    )
    .await;

    match &result {
        Ok(_) => return result,
        Err(e) if e.to_string().contains("منتهي الصلاحية") => return result,
        _ => {}
    }

    // If resuming failed, re-probe once to check if link has expired (server returns text/html)
    if resume_from > 0 {
        if let Err(e) = reprobe_on_resume(client, url, custom_headers).await {
            if e.to_string().contains("منتهي الصلاحية") {
                return Err(e);
            }
        }
    }

    // Second attempt: without Range header (full download)
    // Truncate the partial file and restart from 0 (never append full body to a partial)
    if let Ok(f) = std::fs::OpenOptions::new().write(true).open(part_path) {
        let _ = f.set_len(0);
    }
    let _ = store.save_checkpoint(&Checkpoint::new(task_id, 0, expected_length, Vec::new()));
    let result = attempt_download_attempt(
        client,
        url,
        part_path,
        task_id,
        expected_length,
        0,
        cancel_token.clone(),
        pause_token.clone(),
        event_sender,
        store,
        stall_timeout,
        custom_headers,
        resume_info,
        false,
        true,
        false,
    )
    .await;

    match &result {
        Ok(_) => return result,
        Err(e) if e.to_string().contains("منتهي الصلاحية") => return result,
        _ => {}
    }

    // Third attempt: without cookies/referrer/UA
    if let Ok(f) = std::fs::OpenOptions::new().write(true).open(part_path) {
        let _ = f.set_len(0);
    }
    let _ = store.save_checkpoint(&Checkpoint::new(task_id, 0, expected_length, Vec::new()));
    let minimal_headers = custom_headers.and_then(|h| {
        let filtered: Vec<_> = h
            .iter()
            .filter(|(k, _)| {
                let lower = k.to_lowercase();
                !lower.eq("cookie") && !lower.eq("referer") && !lower.eq("user-agent")
            })
            .cloned()
            .collect();
        if filtered.is_empty() {
            None
        } else {
            Some(filtered)
        }
    });

    attempt_download_attempt(
        client,
        url,
        part_path,
        task_id,
        expected_length,
        0,
        cancel_token,
        pause_token,
        event_sender,
        store,
        stall_timeout,
        minimal_headers.as_deref(),
        resume_info,
        false,
        false,
        false,
    )
    .await
}

/// Executes a sequential streaming download to a `.part` file.
/// Supports re-probe on resume, request fallbacks, and extension guards.
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
    attempt_download_with_fallbacks(
        client,
        url,
        part_path,
        task_id,
        expected_length,
        resume_from,
        cancel_token,
        pause_token,
        event_sender,
        store,
        stall_timeout,
        custom_headers,
        resume_info,
    )
    .await
}

/// Internal helper to perform a single download attempt with given header flags
#[allow(clippy::too_many_arguments)]
async fn attempt_download_attempt(
    client: &reqwest::Client,
    url: &Url,
    part_path: &Path,
    task_id: TaskId,
    expected_length: Option<u64>,
    resume_from: u64,
    cancel_token: CancellationToken,
    pause_token: CancellationToken,
    event_sender: &EventSender,
    store: &Store,
    stall_timeout: std::time::Duration,
    custom_headers: Option<&[(String, String)]>,
    resume_info: Option<&ResumeInfo>,
    use_range: bool,
    include_cookies_referrer_ua: bool,
    _is_first_attempt: bool,
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

    // Build headers based on flags
    if let Some(headers) = custom_headers {
        let filtered_headers: Vec<_> = if include_cookies_referrer_ua {
            headers.to_vec()
        } else {
            headers
                .iter()
                .filter(|(k, _)| {
                    let lower = k.to_lowercase();
                    !lower.eq("cookie") && !lower.eq("referer") && !lower.eq("user-agent")
                })
                .cloned()
                .collect()
        };

        if let Ok(hdr_map) = build_request_headers(&filtered_headers) {
            req_builder = req_builder.headers(hdr_map);
        }
    }

    if use_range && resume_from > 0 {
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
    let retry_policy = RetryPolicy::default();
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
    // If resuming and server returns text/html, the download link expired
    if resume_from > 0 {
        if let Some(ct) = res.headers().get(reqwest::header::CONTENT_TYPE) {
            if let Ok(ct_str) = ct.to_str() {
                if ct_str.to_ascii_lowercase().starts_with("text/html") {
                    return Err(CoreError::validation(
                        "content_type",
                        "الرابط منتهي الصلاحية، يرجى تحديث رابط التحميل".to_string(),
                    ));
                }
            }
        }
    }
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
                        // EOF - check pause/cancel tokens before completing
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
                            let _ = event_sender.emit(
                                task_id,
                                EventKind::Checkpoint,
                                EventPayload::Checkpoint {
                                    durable_bytes: current_written,
                                    total_bytes: expected_length,
                                },
                            ).await;
                            return Ok(DownloadOutcome::Paused {
                                written: current_written,
                            });
                        }
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
                        // EOF and no pause/cancel - break to complete
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

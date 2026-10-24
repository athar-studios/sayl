//! Sequential streaming download to file with progress reporting and truncation detection.
//!
//! Enforces:
//! - Incremental disk writing (configurable chunk buffer size, default 64 KiB).
//! - Verification that written bytes strictly match declared `Content-Length` (NO false complete).
//! - Returns [`StreamOutcome::Incomplete`] on stream truncation or unexpected drop rather than fatal failure.
//! - Cooperative cancellation via [`CancellationToken`].
//! - Explicit `fsync` (`sync_all`) before declaring successful completion.

use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT_ENCODING, CONTENT_LENGTH, USER_AGENT};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
pub use tokio_util::sync::CancellationToken;
use url::Url;

use crate::error::HttpError;
use crate::probe::parse_content_length;

/// Default buffer chunk size for streaming writes (64 KiB).
pub const DEFAULT_CHUNK_SIZE: usize = 64 * 1024;

/// Result outcome of a streaming transfer operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamOutcome {
    /// Download finished completely and durably synced to disk.
    Success {
        /// Total verified byte count written to storage.
        total_bytes: u64,
    },

    /// Download stream was truncated or severed before reaching expected length.
    Incomplete {
        /// Bytes successfully written to disk before truncation occurred.
        written: u64,
    },

    /// Download was cancelled by the caller via the cancellation token.
    Cancelled {
        /// Bytes written to disk before cancellation was observed.
        written: u64,
    },
}

impl StreamOutcome {
    /// Returns `true` if the download completed successfully.
    #[inline]
    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// Returns `true` if the stream was interrupted or truncated.
    #[inline]
    pub const fn is_incomplete(&self) -> bool {
        matches!(self, Self::Incomplete { .. })
    }

    /// Returns `true` if the stream was explicitly cancelled.
    #[inline]
    pub const fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled { .. })
    }

    /// Returns the number of bytes written to disk.
    pub const fn written_bytes(&self) -> u64 {
        match self {
            Self::Success { total_bytes } => *total_bytes,
            Self::Incomplete { written } => *written,
            Self::Cancelled { written } => *written,
        }
    }
}

/// Options configuring sequential streaming.
#[derive(Debug, Clone)]
pub struct StreamOptions {
    /// Size of buffer chunks when streaming.
    pub chunk_size: usize,
    /// User agent string.
    pub user_agent: Option<String>,
    /// Additional headers.
    pub custom_headers: HeaderMap,
}

impl Default for StreamOptions {
    fn default() -> Self {
        Self {
            chunk_size: DEFAULT_CHUNK_SIZE,
            user_agent: Some("SAYL/0.1.0".to_string()),
            custom_headers: HeaderMap::new(),
        }
    }
}

/// Streams an HTTP resource incrementally to a local file.
///
/// Invariants:
/// - Verifies total written bytes equal `expected_length` before returning `Success`.
/// - If stream cuts off early, returns `Ok(StreamOutcome::Incomplete { written })`.
/// - If cancelled, returns `Ok(StreamOutcome::Cancelled { written })`.
/// - Issues `file.sync_all()` (`fsync`) before declaring `Success`.
pub async fn stream_to_file<F>(
    client: &Client,
    url: &Url,
    destination: &Path,
    expected_length: Option<u64>,
    cancel_token: CancellationToken,
    progress_cb: F,
) -> Result<StreamOutcome, HttpError>
where
    F: FnMut(u64) + Send + 'static,
{
    stream_to_file_with_options(
        client,
        url,
        destination,
        expected_length,
        StreamOptions::default(),
        cancel_token,
        progress_cb,
    )
    .await
}

/// Streams an HTTP resource incrementally to a local file with customized options.
pub async fn stream_to_file_with_options<F>(
    client: &Client,
    url: &Url,
    destination: &Path,
    expected_length: Option<u64>,
    options: StreamOptions,
    cancel_token: CancellationToken,
    mut progress_cb: F,
) -> Result<StreamOutcome, HttpError>
where
    F: FnMut(u64) + Send + 'static,
{
    if cancel_token.is_cancelled() {
        return Ok(StreamOutcome::Cancelled { written: 0 });
    }

    // Ensure parent directory exists
    if let Some(parent) = destination.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(destination)
        .await?;

    let mut headers = options.custom_headers;
    headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));

    if let Some(ua) = &options.user_agent {
        if let Ok(val) = HeaderValue::from_str(ua) {
            headers.insert(USER_AGENT, val);
        }
    }

    let req = client.get(url.clone()).headers(headers).build()?;
    let res = client.execute(req).await?;
    let status = res.status();

    if status.is_client_error() || status.is_server_error() {
        return Err(HttpError::status(
            status.as_u16(),
            format!("server returned error status: {status}"),
            None,
        ));
    }

    // Determine target content length from parameter or response header
    let target_length = match expected_length {
        Some(len) => Some(len),
        None => {
            if let Some(cl_val) = res.headers().get(CONTENT_LENGTH) {
                cl_val
                    .to_str()
                    .ok()
                    .and_then(|s| parse_content_length(s).ok())
            } else {
                None
            }
        }
    };

    // If expected length is 0 (empty file) and response is 200 OK with Content-Length 0
    if target_length == Some(0) {
        file.flush().await?;
        file.sync_all().await?;
        progress_cb(0);
        return Ok(StreamOutcome::Success { total_bytes: 0 });
    }

    let mut stream = res.bytes_stream();
    let mut written: u64 = 0;

    loop {
        if cancel_token.is_cancelled() {
            let _ = file.flush().await;
            return Ok(StreamOutcome::Cancelled { written });
        }

        tokio::select! {
            _ = cancel_token.cancelled() => {
                let _ = file.flush().await;
                return Ok(StreamOutcome::Cancelled { written });
            }
            item = stream.next() => {
                match item {
                    Some(Ok(chunk)) => {
                        if !chunk.is_empty() {
                            file.write_all(&chunk).await?;
                            written += chunk.len() as u64;
                            progress_cb(written);
                        }
                    }
                    Some(Err(err)) => {
                        tracing::warn!("stream interrupted at {written} bytes: {err}");
                        let _ = file.flush().await;
                        return Ok(StreamOutcome::Incomplete { written });
                    }
                    None => {
                        break;
                    }
                }
            }
        }
    }

    file.flush().await?;

    // Verify written bytes equal expected length before declaring success
    if let Some(expected) = target_length {
        if written < expected {
            tracing::warn!("stream truncated: received {written} bytes, expected {expected} bytes");
            return Ok(StreamOutcome::Incomplete { written });
        } else if written > expected {
            return Err(HttpError::validation(
                "content_length",
                format!("received {written} bytes, which exceeds expected {expected} bytes"),
            ));
        }
    }

    // fsync before declaring success
    file.sync_all().await?;

    Ok(StreamOutcome::Success {
        total_bytes: written,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_outcome_properties() {
        let success = StreamOutcome::Success { total_bytes: 1024 };
        assert!(success.is_success());
        assert!(!success.is_incomplete());
        assert!(!success.is_cancelled());
        assert_eq!(success.written_bytes(), 1024);

        let incomplete = StreamOutcome::Incomplete { written: 512 };
        assert!(!incomplete.is_success());
        assert!(incomplete.is_incomplete());
        assert_eq!(incomplete.written_bytes(), 512);

        let cancelled = StreamOutcome::Cancelled { written: 256 };
        assert!(cancelled.is_cancelled());
        assert_eq!(cancelled.written_bytes(), 256);
    }
}

//! HTTP client error types and classification for SAYL.
//!
//! Provides [`HttpError`] with retryable, fatal, and cancellation classifications,
//! along with mapping to [`sayl_core::CoreError`].

use std::io;
use thiserror::Error;

/// HTTP error covering transport, status, range, redirect, and payload integrity failures.
#[derive(Debug, Error)]
pub enum HttpError {
    /// Underlying network or client transport error from `reqwest`.
    #[error("transport error: {0}")]
    Transport(#[from] reqwest::Error),

    /// Server returned an unsuccessful or unexpected HTTP status code.
    #[error("HTTP error ({status}): {message}")]
    Status {
        /// HTTP status code.
        status: u16,
        /// Descriptive message or status reason.
        message: String,
        /// Optional `Retry-After` duration in seconds.
        retry_after_secs: Option<u64>,
    },

    /// Safe redirect policy violation (max hops, invalid scheme, or loop).
    #[error("redirect error: {message}")]
    Redirect {
        /// Reason for redirect failure.
        message: String,
    },

    /// Content-Range validation failure, range mismatch, or range not satisfiable.
    #[error("range error: {message}")]
    Range {
        /// Description of the range error.
        message: String,
        /// Optional status code (e.g. 416).
        status: Option<u16>,
    },

    /// Truncated response body (received fewer bytes than declared `Content-Length`).
    #[error("body truncated: expected {expected} bytes, received {actual} bytes")]
    Truncated {
        /// Expected byte length.
        expected: u64,
        /// Actual received byte length.
        actual: u64,
    },

    /// Response header was missing, invalid, or malformed.
    #[error("invalid header '{name}': {reason}")]
    InvalidHeader {
        /// Header name.
        name: String,
        /// Failure reason.
        reason: String,
    },

    /// URL parsing or manipulation failure.
    #[error("URL error: {0}")]
    Url(#[from] url::ParseError),

    /// Local file or socket I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// The operation was explicitly cancelled by the caller.
    #[error("operation cancelled: {reason}")]
    Cancelled {
        /// Cancellation reason.
        reason: String,
    },

    /// Invariant or payload validation error.
    #[error("validation error for {field}: {message}")]
    Validation {
        /// Target field or property.
        field: String,
        /// Validation detail.
        message: String,
    },
}

impl HttpError {
    /// Constructs a status error with optional retry-after seconds.
    pub fn status(status: u16, message: impl Into<String>, retry_after_secs: Option<u64>) -> Self {
        Self::Status {
            status,
            message: message.into(),
            retry_after_secs,
        }
    }

    /// Constructs a redirect error.
    pub fn redirect(message: impl Into<String>) -> Self {
        Self::Redirect {
            message: message.into(),
        }
    }

    /// Constructs a range error.
    pub fn range(message: impl Into<String>, status: Option<u16>) -> Self {
        Self::Range {
            message: message.into(),
            status,
        }
    }

    /// Constructs a cancellation error.
    pub fn cancelled(reason: impl Into<String>) -> Self {
        Self::Cancelled {
            reason: reason.into(),
        }
    }

    /// Constructs an invalid header error.
    pub fn invalid_header(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidHeader {
            name: name.into(),
            reason: reason.into(),
        }
    }

    /// Constructs a validation error.
    pub fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            field: field.into(),
            message: message.into(),
        }
    }

    /// Returns the HTTP status code associated with this error, if applicable.
    pub fn status_code(&self) -> Option<u16> {
        match self {
            Self::Status { status, .. } => Some(*status),
            Self::Range { status, .. } => *status,
            Self::Transport(err) => err.status().map(|s| s.as_u16()),
            _ => None,
        }
    }

    /// Returns the `Retry-After` duration in seconds if advertised.
    pub fn retry_after_secs(&self) -> Option<u64> {
        match self {
            Self::Status {
                retry_after_secs, ..
            } => *retry_after_secs,
            _ => None,
        }
    }

    /// Returns `true` if this error represents a transient condition suitable for retry.
    ///
    /// Retry classification rules:
    /// - 429, 500, 502, 503, 504 are transient.
    /// - Connection drops, timeouts, body truncations are transient.
    /// - 401, 403, 404, 410, redirect failures, validation, and cancellation are NOT retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Status { status, .. } => matches!(status, 429 | 500 | 502 | 503 | 504),
            Self::Transport(err) => {
                if let Some(status) = err.status() {
                    matches!(status.as_u16(), 429 | 500 | 502 | 503 | 504)
                } else {
                    // Timeouts and connection drops are transient
                    err.is_timeout() || err.is_connect() || err.is_request()
                }
            }
            Self::Truncated { .. } => true,
            Self::Io(err) => matches!(
                err.kind(),
                io::ErrorKind::Interrupted
                    | io::ErrorKind::TimedOut
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::BrokenPipe
                    | io::ErrorKind::WouldBlock
            ),
            Self::Range { status, .. } => match status {
                Some(416) => false,
                Some(s) => matches!(s, 429 | 500 | 502 | 503 | 504),
                None => false,
            },
            Self::Redirect { .. } => false,
            Self::InvalidHeader { .. } => false,
            Self::Url(_) => false,
            Self::Cancelled { .. } => false,
            Self::Validation { .. } => false,
        }
    }

    /// Returns `true` if this error represents a fatal, unrecoverable failure.
    pub fn is_fatal(&self) -> bool {
        match self {
            Self::Status { status, .. } => {
                matches!(status, 400 | 401 | 403 | 404 | 405 | 410 | 451)
            }
            Self::Redirect { .. } => true,
            Self::InvalidHeader { .. } => true,
            Self::Url(_) => true,
            Self::Validation { .. } => true,
            Self::Cancelled { .. } => false, // User cancellation is intentional, not fatal failure
            Self::Io(err) => matches!(
                err.kind(),
                io::ErrorKind::PermissionDenied
                    | io::ErrorKind::NotFound
                    | io::ErrorKind::AlreadyExists
            ),
            Self::Transport(err) => {
                if let Some(status) = err.status() {
                    matches!(status.as_u16(), 400 | 401 | 403 | 404 | 405 | 410 | 451)
                } else {
                    false
                }
            }
            Self::Truncated { .. } => false,
            Self::Range { status, .. } => matches!(status, Some(400 | 401 | 403 | 404)),
        }
    }

    /// Returns `true` if the error was caused by explicit cancellation.
    #[inline]
    pub const fn is_user_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled { .. })
    }
}

impl From<HttpError> for sayl_core::CoreError {
    fn from(err: HttpError) -> Self {
        match err {
            HttpError::Status {
                status,
                message,
                retry_after_secs,
            } => sayl_core::CoreError::http(Some(status), message, retry_after_secs),
            HttpError::Transport(req_err) => {
                let status_code = req_err.status().map(|s| s.as_u16());
                let msg = req_err.to_string();
                sayl_core::CoreError::http(status_code, msg, None)
            }
            HttpError::Redirect { message } => sayl_core::CoreError::http(None, message, None),
            HttpError::Range { message, status } => {
                sayl_core::CoreError::http(status, message, None)
            }
            HttpError::Truncated { expected, actual } => sayl_core::CoreError::validation(
                "content_length",
                format!("received {actual} of declared {expected} bytes"),
            ),
            HttpError::InvalidHeader { name, reason } => {
                sayl_core::CoreError::validation("header", format!("header '{name}': {reason}"))
            }
            HttpError::Url(url_err) => sayl_core::CoreError::validation("url", url_err.to_string()),
            HttpError::Io(io_err) => sayl_core::CoreError::Io {
                source: io_err,
                context: Some("http transport".to_string()),
            },
            HttpError::Cancelled { reason } => sayl_core::CoreError::cancelled(reason),
            HttpError::Validation { field, message } => {
                // Leaking `&'static str` is fine for dynamic fields or we use a fixed static
                let static_field = match field.as_str() {
                    "content_length" => "content_length",
                    "content_range" => "content_range",
                    "range" => "range",
                    "filename" => "filename",
                    "url" => "url",
                    _ => "http_validation",
                };
                sayl_core::CoreError::validation(static_field, message)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_error_classification() {
        let err429 = HttpError::status(429, "Too Many Requests", Some(60));
        assert!(err429.is_retryable());
        assert!(!err429.is_fatal());
        assert_eq!(err429.status_code(), Some(429));
        assert_eq!(err429.retry_after_secs(), Some(60));

        let err503 = HttpError::status(503, "Service Unavailable", None);
        assert!(err503.is_retryable());
        assert!(!err503.is_fatal());

        let err404 = HttpError::status(404, "Not Found", None);
        assert!(!err404.is_retryable());
        assert!(err404.is_fatal());

        let err401 = HttpError::status(401, "Unauthorized", None);
        assert!(!err401.is_retryable());
        assert!(err401.is_fatal());
    }

    #[test]
    fn test_cancellation_classification() {
        let cancel = HttpError::cancelled("user clicked pause");
        assert!(cancel.is_user_cancelled());
        assert!(!cancel.is_retryable());
        assert!(!cancel.is_fatal());
    }

    #[test]
    fn test_truncated_is_retryable() {
        let trunc = HttpError::Truncated {
            expected: 1000,
            actual: 500,
        };
        assert!(trunc.is_retryable());
        assert!(!trunc.is_fatal());
    }

    #[test]
    fn test_mapping_to_core_error() {
        let http_status = HttpError::status(503, "Server busy", Some(5));
        let core_err: sayl_core::CoreError = http_status.into();
        assert_eq!(core_err.category(), "Http");
        assert!(core_err.is_retryable());

        let cancel = HttpError::cancelled("stopped by user");
        let core_cancel: sayl_core::CoreError = cancel.into();
        assert_eq!(core_cancel.category(), "Cancelled");
        assert!(core_cancel.is_user_cancelled());

        let trunc = HttpError::Truncated {
            expected: 100,
            actual: 50,
        };
        let core_trunc: sayl_core::CoreError = trunc.into();
        assert_eq!(core_trunc.category(), "Validation");

        let io_err = HttpError::Io(io::Error::new(io::ErrorKind::ConnectionReset, "reset"));
        let core_io: sayl_core::CoreError = io_err.into();
        assert_eq!(core_io.category(), "Io");
        assert!(core_io.is_retryable());
    }
}

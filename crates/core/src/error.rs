//! Core error types and classifications.
//!
//! Provides [`CoreError`] covering the seven failure categories defined in
//! `docs/tasks/CORE-001.md`: `Http`, `Io`, `Storage`, `Scheduler`, `Validation`,
//! `Cancelled`, and `Internal`.

use std::fmt;
use std::io;

use crate::state::StateTransitionError;

/// Core error enum covering all engine and lifecycle failure domains.
#[derive(Debug)]
pub enum CoreError {
    /// Network or HTTP protocol level failures (status codes, timeouts, redirects).
    Http {
        /// HTTP status code if a response was received.
        status_code: Option<u16>,
        /// Descriptive error message.
        message: String,
        /// Optional Retry-After duration in seconds indicated by the origin.
        retry_after_secs: Option<u64>,
        /// Whether the error represents a connection/network timeout or reset.
        is_transient: bool,
    },
    /// Operating system I/O error (disk, socket, pipe).
    Io {
        /// Underlying I/O error.
        source: io::Error,
        /// Contextual message where the I/O failure occurred.
        context: Option<String>,
    },
    /// SQLite or metadata storage layer failures.
    Storage {
        /// Context or storage operation that failed.
        operation: &'static str,
        /// Detail or message.
        message: String,
    },
    /// Scheduler, worker coordination, lease expiry, or splitting errors.
    Scheduler {
        /// Reason for the scheduler fault.
        reason: String,
    },
    /// Invariant, range bounds, checksum, or input validation failures.
    Validation {
        /// Target field, range, or property that failed validation.
        field: &'static str,
        /// Validation failure detail.
        message: String,
    },
    /// Task was explicitly cancelled by user command or application shutdown.
    Cancelled {
        /// Reason or initiator for cancellation.
        reason: String,
    },
    /// Internal unexpected faults or invariant violations.
    Internal {
        /// Error message describing the unexpected fault.
        message: String,
    },
}

impl CoreError {
    /// Creates an HTTP error.
    pub fn http(
        status_code: Option<u16>,
        message: impl Into<String>,
        retry_after: Option<u64>,
    ) -> Self {
        let is_transient = match status_code {
            Some(429 | 500 | 502 | 503 | 504) => true,
            Some(_) => false,
            None => true, // Connection drops / timeouts without status code
        };
        Self::Http {
            status_code,
            message: message.into(),
            retry_after_secs: retry_after,
            is_transient,
        }
    }

    /// Creates an I/O error with context.
    pub fn io_with_context(source: io::Error, context: impl Into<String>) -> Self {
        Self::Io {
            source,
            context: Some(context.into()),
        }
    }

    /// Creates a Storage error.
    pub fn storage(operation: &'static str, message: impl Into<String>) -> Self {
        Self::Storage {
            operation,
            message: message.into(),
        }
    }

    /// Creates a Scheduler error.
    pub fn scheduler(reason: impl Into<String>) -> Self {
        Self::Scheduler {
            reason: reason.into(),
        }
    }

    /// Creates a Validation error.
    pub fn validation(field: &'static str, message: impl Into<String>) -> Self {
        Self::Validation {
            field,
            message: message.into(),
        }
    }

    /// Creates a user or system cancellation error.
    pub fn cancelled(reason: impl Into<String>) -> Self {
        Self::Cancelled {
            reason: reason.into(),
        }
    }

    /// Creates an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Returns the category name of the error.
    pub const fn category(&self) -> &'static str {
        match self {
            Self::Http { .. } => "Http",
            Self::Io { .. } => "Io",
            Self::Storage { .. } => "Storage",
            Self::Scheduler { .. } => "Scheduler",
            Self::Validation { .. } => "Validation",
            Self::Cancelled { .. } => "Cancelled",
            Self::Internal { .. } => "Internal",
        }
    }

    /// Returns `true` if this error is transient and safe to retry according to policy.
    ///
    /// Following `docs/MASTER-PLAN.md` §6:
    /// - 429, 503, network drops, timeouts, and transient I/O are retryable with backoff.
    /// - 401, 403, 404, validation errors, and cancellation are NOT retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Http { is_transient, .. } => *is_transient,
            Self::Io { source, .. } => matches!(
                source.kind(),
                io::ErrorKind::Interrupted
                    | io::ErrorKind::TimedOut
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::BrokenPipe
                    | io::ErrorKind::WouldBlock
            ),
            Self::Scheduler { .. } => false,
            Self::Storage { .. } => false,
            Self::Validation { .. } => false,
            Self::Cancelled { .. } => false,
            Self::Internal { .. } => false,
        }
    }

    /// Returns `true` if this error represents an unrecoverable, fatal termination.
    pub fn is_fatal(&self) -> bool {
        match self {
            Self::Http { status_code, .. } => matches!(status_code, Some(401 | 403 | 404 | 410)),
            Self::Validation { .. } => true,
            Self::Storage { .. } => true,
            Self::Cancelled { .. } => false, // Cancelled is user action, not failure
            Self::Io { source, .. } => matches!(
                source.kind(),
                io::ErrorKind::PermissionDenied
                    | io::ErrorKind::NotFound
                    | io::ErrorKind::AlreadyExists
            ),
            Self::Scheduler { .. } => false,
            Self::Internal { .. } => true,
        }
    }

    /// Returns `true` if this error was explicitly triggered by user cancellation.
    #[inline]
    pub const fn is_user_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled { .. })
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http {
                status_code,
                message,
                retry_after_secs,
                ..
            } => {
                write!(f, "HTTP error")?;
                if let Some(code) = status_code {
                    write!(f, " ({code})")?;
                }
                write!(f, ": {message}")?;
                if let Some(ra) = retry_after_secs {
                    write!(f, " [Retry-After: {ra}s]")?;
                }
                Ok(())
            }
            Self::Io { source, context } => {
                if let Some(ctx) = context {
                    write!(f, "I/O error during {ctx}: {source}")
                } else {
                    write!(f, "I/O error: {source}")
                }
            }
            Self::Storage { operation, message } => {
                write!(f, "Storage error during {operation}: {message}")
            }
            Self::Scheduler { reason } => {
                write!(f, "Scheduler error: {reason}")
            }
            Self::Validation { field, message } => {
                write!(f, "Validation error for {field}: {message}")
            }
            Self::Cancelled { reason } => {
                write!(f, "Operation cancelled: {reason}")
            }
            Self::Internal { message } => {
                write!(f, "Internal error: {message}")
            }
        }
    }
}

impl std::error::Error for CoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<io::Error> for CoreError {
    #[inline]
    fn from(source: io::Error) -> Self {
        Self::Io {
            source,
            context: None,
        }
    }
}

impl From<StateTransitionError> for CoreError {
    #[inline]
    fn from(err: StateTransitionError) -> Self {
        Self::Validation {
            field: "state_transition",
            message: err.to_string(),
        }
    }
}

impl From<String> for CoreError {
    #[inline]
    fn from(message: String) -> Self {
        Self::Internal { message }
    }
}

impl From<&str> for CoreError {
    #[inline]
    fn from(message: &str) -> Self {
        Self::Internal {
            message: message.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categories_and_display() {
        let http_err = CoreError::http(Some(503), "Service Unavailable", Some(5));
        assert_eq!(http_err.category(), "Http");
        assert!(http_err.to_string().contains("503"));
        assert!(http_err.to_string().contains("Retry-After: 5s"));

        let io_err =
            CoreError::io_with_context(io::Error::from(io::ErrorKind::NotFound), "file write");
        assert_eq!(io_err.category(), "Io");
        assert!(io_err.to_string().contains("file write"));

        let storage_err = CoreError::storage("checkpoint", "sqlite disk full");
        assert_eq!(storage_err.category(), "Storage");

        let sched_err = CoreError::scheduler("lease generation conflict");
        assert_eq!(sched_err.category(), "Scheduler");

        let val_err = CoreError::validation("range", "end <= start");
        assert_eq!(val_err.category(), "Validation");

        let cancel_err = CoreError::cancelled("user clicked pause/cancel");
        assert_eq!(cancel_err.category(), "Cancelled");

        let int_err = CoreError::internal("unreachable code path");
        assert_eq!(int_err.category(), "Internal");
    }

    #[test]
    fn test_is_retryable() {
        assert!(CoreError::http(Some(429), "Too Many Requests", None).is_retryable());
        assert!(CoreError::http(Some(503), "Unavailable", None).is_retryable());
        assert!(CoreError::http(None, "Connection Reset", None).is_retryable());
        assert!(!CoreError::http(Some(401), "Unauthorized", None).is_retryable());
        assert!(!CoreError::http(Some(403), "Forbidden", None).is_retryable());
        assert!(!CoreError::http(Some(404), "Not Found", None).is_retryable());

        let timeout_io = CoreError::from(io::Error::from(io::ErrorKind::TimedOut));
        assert!(timeout_io.is_retryable());

        let perm_io = CoreError::from(io::Error::from(io::ErrorKind::PermissionDenied));
        assert!(!perm_io.is_retryable());

        assert!(!CoreError::validation("checksum", "mismatch").is_retryable());
        assert!(!CoreError::cancelled("stopped").is_retryable());
    }

    #[test]
    fn test_is_fatal() {
        assert!(CoreError::http(Some(401), "Unauthorized", None).is_fatal());
        assert!(CoreError::http(Some(403), "Forbidden", None).is_fatal());
        assert!(CoreError::validation("hash", "sha256 mismatch").is_fatal());
        assert!(CoreError::storage("sync", "corrupted page").is_fatal());
        assert!(CoreError::from(io::Error::from(io::ErrorKind::PermissionDenied)).is_fatal());

        // Transient errors are not fatal
        assert!(!CoreError::http(Some(503), "Busy", None).is_fatal());
        assert!(!CoreError::from(io::Error::from(io::ErrorKind::TimedOut)).is_fatal());
    }

    #[test]
    fn test_is_user_cancelled() {
        let cancel = CoreError::cancelled("user stopped");
        assert!(cancel.is_user_cancelled());

        let other = CoreError::http(Some(500), "Server error", None);
        assert!(!other.is_user_cancelled());
    }

    #[test]
    fn test_conversions() {
        let io_err = io::Error::new(io::ErrorKind::BrokenPipe, "pipe broke");
        let core_err: CoreError = io_err.into();
        assert_eq!(core_err.category(), "Io");
        assert!(std::error::Error::source(&core_err).is_some());

        let trans_err = StateTransitionError {
            from: crate::state::DownloadState::Completed,
            to: crate::state::DownloadState::Queued,
        };
        let val_err: CoreError = trans_err.into();
        assert_eq!(val_err.category(), "Validation");

        let str_err: CoreError = "something failed".into();
        assert_eq!(str_err.category(), "Internal");
    }
}

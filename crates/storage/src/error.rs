//! Storage error taxonomy and CoreError mapping.
//!
//! Provides [`StoreError`] covering I/O, serialization, corruption, missing entities,
//! and lock contention, with seamless conversion into [`sayl_core::CoreError`].

use sayl_core::error::CoreError;
use sayl_core::id::TaskId;

/// Storage-specific errors encountered during journal or snapshot operations.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// File system or disk I/O failure.
    #[error("Storage I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization or deserialization failure.
    #[error("Storage serde error: {0}")]
    Serde(#[from] serde_json::Error),

    /// Storage corruption detected during journal replay or snapshot loading.
    #[error("Storage corruption detected: {0}")]
    Corruption(String),

    /// Requested task or checkpoint does not exist.
    #[error("Entity not found for task ID: {0}")]
    NotFound(TaskId),

    /// Storage directory is locked by another active Store instance.
    #[error("Storage directory is locked: {0}")]
    Locked(String),

    /// Other unclassified storage failure.
    #[error("Storage error: {0}")]
    Other(String),
}

impl StoreError {
    /// Converts this storage error into an engine-level [`CoreError`].
    pub fn to_core_error(&self) -> CoreError {
        match self {
            Self::Io(err) => {
                let io_copy = std::io::Error::new(err.kind(), err.to_string());
                CoreError::io_with_context(io_copy, "sayl-storage")
            }
            Self::Serde(err) => CoreError::storage("serde", err.to_string()),
            Self::Corruption(msg) => CoreError::storage("corruption", msg.clone()),
            Self::NotFound(id) => CoreError::storage("not_found", format!("task {id} not found")),
            Self::Locked(msg) => CoreError::storage("locked", msg.clone()),
            Self::Other(msg) => CoreError::storage("other", msg.clone()),
        }
    }
}

impl From<StoreError> for CoreError {
    fn from(err: StoreError) -> Self {
        match err {
            StoreError::Io(e) => CoreError::io_with_context(e, "sayl-storage"),
            StoreError::Serde(e) => CoreError::storage("serde", e.to_string()),
            StoreError::Corruption(msg) => CoreError::storage("corruption", msg),
            StoreError::NotFound(id) => {
                CoreError::storage("not_found", format!("task {id} not found"))
            }
            StoreError::Locked(msg) => CoreError::storage("locked", msg),
            StoreError::Other(msg) => CoreError::storage("other", msg),
        }
    }
}

/// Convenience result alias for storage operations.
pub type Result<T> = std::result::Result<T, StoreError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_and_core_mapping() {
        let task_id = TaskId::new();
        let not_found = StoreError::NotFound(task_id);
        assert!(not_found.to_string().contains(&task_id.to_string()));

        let core_err: CoreError = not_found.into();
        assert_eq!(core_err.category(), "Storage");
        assert!(core_err.to_string().contains("not_found"));

        let corrupt = StoreError::Corruption("bad magic header".to_string());
        let core_corrupt: CoreError = corrupt.into();
        assert_eq!(core_corrupt.category(), "Storage");

        let locked = StoreError::Locked("dir locked".to_string());
        let core_locked = locked.to_core_error();
        assert_eq!(core_locked.category(), "Storage");

        let io_err = StoreError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "disk read error",
        ));
        let core_io: CoreError = io_err.into();
        assert_eq!(core_io.category(), "Io");
    }
}

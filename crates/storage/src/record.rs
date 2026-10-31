//! Storage record models for tasks and checkpoints.
//!
//! Provides [`TaskRecord`] and [`Checkpoint`] models serialized to JSON
//! for snapshotting and write-ahead journaling.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use sayl_core::id::TaskId;
use sayl_core::state::DownloadState;

/// Persistent record representing a download task and its lifecycle metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRecord {
    /// Strongly-typed time-ordered unique task identifier.
    pub id: TaskId,
    /// Initial target URL requested by the user.
    pub url: String,
    /// Final resolved redirect URL, if any.
    pub final_url: Option<String>,
    /// Target destination directory or path on local storage.
    pub save_path: PathBuf,
    /// Destination file name on disk.
    pub filename: String,
    /// Current lifecycle state of the download task.
    pub state: DownloadState,
    /// Total expected byte size, or `None` if unknown (chunked or unprobed).
    pub total_size: Option<u64>,
    /// Total number of bytes written durably to disk so far.
    pub written: u64,
    /// HTTP ETag validator from origin response headers.
    pub etag: Option<String>,
    /// HTTP Last-Modified validator from origin response headers.
    pub last_modified: Option<String>,
    /// MIME content type from origin response headers.
    #[serde(default)]
    pub content_type: Option<String>,
    /// Timestamp when the task was initially registered.
    pub created_at: DateTime<Utc>,
    /// Timestamp when the task was last updated or checkpointed.
    pub updated_at: DateTime<Utc>,
    /// Last error message if in `Failed` state.
    pub error: Option<String>,
    /// Classification kind of the error (e.g. "http_429", "too_many_redirects").
    #[serde(default)]
    pub error_kind: Option<String>,
    /// Optional per-task speed limit in KB/s. `None` or `0` means unlimited.
    #[serde(default)]
    pub speed_limit_kbps: Option<u32>,
}

impl TaskRecord {
    /// Creates a new [`TaskRecord`] in [`DownloadState::Queued`] state.
    pub fn new(
        url: impl Into<String>,
        save_path: impl Into<PathBuf>,
        filename: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: TaskId::new(),
            url: url.into(),
            final_url: None,
            save_path: save_path.into(),
            filename: filename.into(),
            state: DownloadState::Queued,
            total_size: None,
            written: 0,
            etag: None,
            last_modified: None,
            content_type: None,
            created_at: now,
            updated_at: now,
            error: None,
            error_kind: None,
            speed_limit_kbps: None,
        }
    }

    /// Creates a new [`TaskRecord`] with an explicit [`TaskId`].
    pub fn with_id(
        id: TaskId,
        url: impl Into<String>,
        save_path: impl Into<PathBuf>,
        filename: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            url: url.into(),
            final_url: None,
            save_path: save_path.into(),
            filename: filename.into(),
            state: DownloadState::Queued,
            total_size: None,
            written: 0,
            etag: None,
            last_modified: None,
            content_type: None,
            created_at: now,
            updated_at: now,
            error: None,
            error_kind: None,
            speed_limit_kbps: None,
        }
    }
}

/// Durable checkpoint capturing progress and byte range allocations.
///
/// Designed to support parallel multi-connection downloads by recording
/// discrete `(start, end)` byte ranges that have been durably written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Target task identifier associated with this checkpoint.
    pub task_id: TaskId,
    /// Total cumulative bytes written across all ranges.
    pub written: u64,
    /// Total expected byte size of the file, if known.
    pub total_size: Option<u64>,
    /// List of completed or assigned byte ranges `(start_offset, end_offset)`.
    pub range_map: Vec<(u64, u64)>,
    /// Timestamp when this checkpoint was captured.
    pub updated_at: DateTime<Utc>,
}

impl Checkpoint {
    /// Creates a new checkpoint record for a given task.
    pub fn new(
        task_id: TaskId,
        written: u64,
        total_size: Option<u64>,
        range_map: Vec<(u64, u64)>,
    ) -> Self {
        Self {
            task_id,
            written,
            total_size,
            range_map,
            updated_at: Utc::now(),
        }
    }

    /// Adds a byte range `(start, end)` to the checkpoint's range map.
    pub fn add_range(&mut self, start: u64, end: u64) {
        self.range_map.push((start, end));
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_record_serde_roundtrip() {
        let record = TaskRecord::new(
            "https://example.com/file.iso",
            PathBuf::from("/downloads"),
            "file.iso",
        );

        let json = serde_json::to_string(&record).expect("serialization succeeds");
        let decoded: TaskRecord = serde_json::from_str(&json).expect("deserialization succeeds");
        assert_eq!(record, decoded);
    }

    #[test]
    fn test_task_record_deserialize_without_content_type() {
        let json = r#"{
            "id": "01900000-0000-7000-8000-000000000001",
            "url": "https://example.com/file.iso",
            "final_url": null,
            "save_path": "/downloads",
            "filename": "file.iso",
            "state": "Queued",
            "total_size": null,
            "written": 0,
            "etag": null,
            "last_modified": null,
            "created_at": "2025-01-01T00:00:00Z",
            "updated_at": "2025-01-01T00:00:00Z",
            "error": null
        }"#;

        let decoded: TaskRecord = serde_json::from_str(json).expect("deserialization succeeds");
        assert_eq!(decoded.content_type, None);
    }

    #[test]
    fn test_checkpoint_multi_range_serde_roundtrip() {
        let task_id = TaskId::new();
        let ranges = vec![(0, 1024), (1024, 2048), (2048, 4096), (4096, 1000000)];
        let checkpoint = Checkpoint::new(task_id, 1000000, Some(1000000), ranges.clone());

        let json = serde_json::to_string(&checkpoint).expect("serialization succeeds");
        let decoded: Checkpoint = serde_json::from_str(&json).expect("deserialization succeeds");
        assert_eq!(checkpoint, decoded);
        assert_eq!(decoded.range_map, ranges);
    }
}

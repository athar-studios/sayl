//! Task identifier representation based on UUID v7.
//!
//! Provides a strongly typed [`TaskId`] wrapper around time-ordered UUID v7 values,
//! ensuring chronological sorting and uniqueness for download tasks.

use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Unique identifier for a download task, backed by a timestamp-ordered UUID v7.
///
/// UUID v7 guarantees chronological monotonicity in typical operation, enabling
/// time-sorted task storage and log aggregation without separate timestamp indexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct TaskId(Uuid);

impl TaskId {
    /// Generates a new task identifier with the current timestamp using UUID v7.
    #[inline]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Creates a task identifier wrapping an existing [`Uuid`].
    #[inline]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns a reference to the underlying [`Uuid`].
    #[inline]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Consumes the wrapper and returns the underlying [`Uuid`].
    #[inline]
    pub fn into_inner(self) -> Uuid {
        self.0
    }

    /// Returns the millisecond Unix timestamp embedded within the UUID v7.
    ///
    /// Returns `None` if the underlying UUID is not a version 7 UUID.
    pub fn timestamp_millis(&self) -> Option<u64> {
        self.0.get_timestamp().map(|ts| {
            let (secs, nanos) = ts.to_unix();
            secs * 1000 + (nanos as u64) / 1_000_000
        })
    }
}

impl Default for TaskId {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TaskId {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl FromStr for TaskId {
    type Err = uuid::Error;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::from_str(s).map(Self)
    }
}

impl From<Uuid> for TaskId {
    #[inline]
    fn from(uuid: Uuid) -> Self {
        Self::from_uuid(uuid)
    }
}

impl From<TaskId> for Uuid {
    #[inline]
    fn from(task_id: TaskId) -> Self {
        task_id.into_inner()
    }
}

impl AsRef<Uuid> for TaskId {
    #[inline]
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_id_creation_and_ordering() {
        let id1 = TaskId::new();
        let id2 = TaskId::new();

        // Check that two generated IDs are distinct
        assert_ne!(id1, id2);

        // UUID v7 timestamps are non-decreasing
        assert!(id1 <= id2);

        // Version should be 7
        assert_eq!(id1.as_uuid().get_version_num(), 7);
        assert_eq!(id2.as_uuid().get_version_num(), 7);
    }

    #[test]
    fn test_task_id_display_and_from_str() {
        let original = TaskId::new();
        let formatted = original.to_string();

        let parsed =
            TaskId::from_str(&formatted).expect("parsing valid UUID string should succeed");
        assert_eq!(original, parsed);
        assert_eq!(parsed.to_string(), formatted);
    }

    #[test]
    fn test_task_id_invalid_from_str() {
        assert!(TaskId::from_str("invalid-uuid-string").is_err());
        assert!(TaskId::from_str("").is_err());
        assert!(TaskId::from_str("00000000-0000-0000-0000-00000000000g").is_err());
    }

    #[test]
    fn test_task_id_from_into_uuid() {
        let uuid = Uuid::now_v7();
        let task_id: TaskId = uuid.into();
        assert_eq!(task_id.as_uuid(), &uuid);

        let recovered: Uuid = task_id.into();
        assert_eq!(recovered, uuid);
    }

    #[test]
    fn test_task_id_timestamp_millis() {
        let id = TaskId::new();
        let ts = id.timestamp_millis();
        assert!(ts.is_some());
        assert!(ts.unwrap() > 1_700_000_000_000); // Beyond late 2023
    }
}

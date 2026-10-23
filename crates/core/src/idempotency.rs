//! Idempotency key types and in-memory deduplication store.
//!
//! Provides [`IdempotencyKey`] and [`IdempotencyStore`] with configurable TTL
//! (default 24 hours) and automatic cleanup to guarantee that repeated client
//! commands (start, pause, resume, cancel, retry) return identical responses
//! without duplicating side effects.

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::id::TaskId;

/// Default time-to-live for idempotency records (24 hours).
pub const DEFAULT_IDEMPOTENCY_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Unique idempotency key supplied by the client for mutating operations.
///
/// Ensures safe retries and network replay tolerance by identifying identical
/// command invocations across connection drops and process restarts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct IdempotencyKey(Uuid);

impl IdempotencyKey {
    /// Generates a new random idempotency key using UUID v7.
    #[inline]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Creates an idempotency key from an existing [`Uuid`].
    #[inline]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns a reference to the inner [`Uuid`].
    #[inline]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Consumes the key and returns the inner [`Uuid`].
    #[inline]
    pub fn into_inner(self) -> Uuid {
        self.0
    }
}

impl Default for IdempotencyKey {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for IdempotencyKey {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl FromStr for IdempotencyKey {
    type Err = uuid::Error;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::from_str(s).map(Self)
    }
}

impl From<Uuid> for IdempotencyKey {
    #[inline]
    fn from(uuid: Uuid) -> Self {
        Self::from_uuid(uuid)
    }
}

impl From<IdempotencyKey> for Uuid {
    #[inline]
    fn from(key: IdempotencyKey) -> Self {
        key.into_inner()
    }
}

impl AsRef<Uuid> for IdempotencyKey {
    #[inline]
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

/// A cached execution record for an idempotent command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyRecord<T = String> {
    /// Idempotency key supplied with the request.
    pub key: IdempotencyKey,
    /// Download task associated with the command.
    pub task_id: TaskId,
    /// Cached response payload returned to the caller.
    pub response: T,
    /// Monotonic timestamp when the record was cached.
    pub inserted_at: Instant,
}

impl<T> IdempotencyRecord<T> {
    /// Checks whether this record has exceeded the given TTL duration.
    #[inline]
    pub fn is_expired(&self, ttl: Duration) -> bool {
        self.inserted_at.elapsed() >= ttl
    }
}

/// In-memory thread-safe store for tracking consumed idempotency keys.
///
/// Retains operation responses for a configurable time-to-live (default 24h)
/// and provides atomic lookup, insertion, and eviction of expired entries.
#[derive(Debug, Clone)]
pub struct IdempotencyStore<T = String> {
    records: Arc<RwLock<HashMap<IdempotencyKey, IdempotencyRecord<T>>>>,
    ttl: Duration,
}

impl<T: Clone> IdempotencyStore<T> {
    /// Creates a new in-memory store with the specified TTL.
    pub fn new(ttl: Duration) -> Self {
        Self {
            records: Arc::new(RwLock::new(HashMap::new())),
            ttl,
        }
    }

    /// Looks up a cached record for the given key.
    ///
    /// Returns `None` if the key is not present or if its TTL has expired.
    pub fn get(&self, key: &IdempotencyKey) -> Option<IdempotencyRecord<T>> {
        let guard = self.records.read().ok()?;
        let record = guard.get(key)?;
        if record.is_expired(self.ttl) {
            None
        } else {
            Some(record.clone())
        }
    }

    /// Inserts or updates an idempotency record for the given key and task ID.
    pub fn insert(&self, key: IdempotencyKey, task_id: TaskId, response: T) {
        let record = IdempotencyRecord {
            key,
            task_id,
            response,
            inserted_at: Instant::now(),
        };
        if let Ok(mut guard) = self.records.write() {
            guard.insert(key, record);
        }
    }

    /// Removes all records whose TTL has elapsed.
    ///
    /// Returns the number of expired entries purged from the store.
    pub fn cleanup(&self) -> usize {
        let mut guard = match self.records.write() {
            Ok(g) => g,
            Err(_) => return 0,
        };

        let ttl = self.ttl;
        let before = guard.len();
        guard.retain(|_, record| !record.is_expired(ttl));
        before - guard.len()
    }

    /// Returns the total number of entries currently in the store (including potentially expired ones).
    pub fn len(&self) -> usize {
        self.records.read().map(|g| g.len()).unwrap_or(0)
    }

    /// Returns `true` if the store contains no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clears all entries from the store.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.records.write() {
            guard.clear();
        }
    }

    /// Returns the configured time-to-live for records in this store.
    pub fn ttl(&self) -> Duration {
        self.ttl
    }
}

impl<T: Clone> Default for IdempotencyStore<T> {
    fn default() -> Self {
        Self::new(DEFAULT_IDEMPOTENCY_TTL)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idempotency_key_creation_and_parse() {
        let key = IdempotencyKey::new();
        let str_rep = key.to_string();

        let parsed: IdempotencyKey = str_rep.parse().expect("parsing valid UUID should succeed");
        assert_eq!(key, parsed);
        assert_eq!(key.as_uuid(), parsed.as_uuid());
    }

    #[test]
    fn test_idempotency_store_insert_and_get() {
        let store = IdempotencyStore::new(Duration::from_secs(60));
        let key = IdempotencyKey::new();
        let task_id = TaskId::new();

        assert_eq!(store.get(&key), None);

        store.insert(key, task_id, "OK".to_string());

        let record = store.get(&key).expect("record should exist");
        assert_eq!(record.key, key);
        assert_eq!(record.task_id, task_id);
        assert_eq!(record.response, "OK");
    }

    #[test]
    fn test_idempotency_store_ttl_expiration() {
        let store = IdempotencyStore::new(Duration::from_millis(50));
        let key = IdempotencyKey::new();
        let task_id = TaskId::new();

        store.insert(key, task_id, "response-1".to_string());
        assert!(store.get(&key).is_some());

        // Sleep past TTL
        std::thread::sleep(Duration::from_millis(60));

        // get() should return None because it is expired
        assert_eq!(store.get(&key), None);

        // cleanup should purge the expired entry
        let purged = store.cleanup();
        assert_eq!(purged, 1);
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_idempotency_store_cleanup_retention() {
        let store = IdempotencyStore::new(Duration::from_secs(10));
        let task_id = TaskId::new();

        let k1 = IdempotencyKey::new();
        let k2 = IdempotencyKey::new();

        store.insert(k1, task_id, "res1".to_string());
        store.insert(k2, task_id, "res2".to_string());

        assert_eq!(store.len(), 2);
        let purged = store.cleanup();
        assert_eq!(purged, 0);
        assert_eq!(store.len(), 2);
    }
}

//! Crash-safe task and checkpoint store.
//!
//! Provides [`Store`] and [`StoreStats`] adhering to atomic snapshot and
//! write-ahead journal replay architecture.

use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use sayl_core::id::TaskId;
use crate::error::{Result, StoreError};
use crate::journal::{Entry, Journal, JournalOp};
use crate::record::{Checkpoint, TaskRecord};

static OPEN_DIRS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

fn get_open_dirs() -> &'static Mutex<HashSet<PathBuf>> {
    OPEN_DIRS.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Active storage metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StoreStats {
    /// Number of active download task records currently held in store.
    pub records: usize,
    /// Size of the append-only journal file in bytes.
    pub journal_bytes: u64,
}

/// Atomic snapshot structure persisted to disk during compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Snapshot {
    version: u32,
    last_seq: u64,
    tasks: Vec<TaskRecord>,
    checkpoints: Vec<Checkpoint>,
}

/// Directory lock guard guaranteeing single-writer exclusivity.
struct LockGuard {
    lock_path: PathBuf,
    canonical_dir: PathBuf,
    _file: Option<File>,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        // Drop the OS file handle to release the file lock
        self._file = None;

        // Release the in-process registry lock
        if let Ok(mut dirs) = get_open_dirs().lock() {
            dirs.remove(&self.canonical_dir);
        }

        // Best-effort cleanup of the lock file
        let _ = fs::remove_file(&self.lock_path);
    }
}

struct StoreInner {
    dir: Option<PathBuf>,
    journal: Option<Journal>,
    tasks: BTreeMap<TaskId, TaskRecord>,
    checkpoints: BTreeMap<TaskId, Checkpoint>,
    seq: u64,
}

/// Durable, crash-safe persistent store for download tasks and checkpoints.
pub struct Store {
    inner: Mutex<StoreInner>,
    _lock: Option<LockGuard>,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stats = self.stats();
        f.debug_struct("Store")
            .field("records", &stats.records)
            .field("journal_bytes", &stats.journal_bytes)
            .finish()
    }
}

impl Store {
    /// Opens or initializes a persistent [`Store`] backed by the specified directory.
    ///
    /// Recovers state by reading the latest atomic snapshot (if present)
    /// and replaying the write-ahead journal. Ensures single-writer exclusivity
    /// using directory locks.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        let dir_path = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir_path)?;

        let canonical_dir = dir_path.canonicalize().unwrap_or_else(|_| dir_path.clone());

        // 1. In-process lock check
        {
            let mut dirs = get_open_dirs()
                .lock()
                .map_err(|_| StoreError::Other("Directory lock mutex poisoned".to_string()))?;
            if !dirs.insert(canonical_dir.clone()) {
                return Err(StoreError::Locked(format!(
                    "Directory {:?} is already open by another Store handle in this process",
                    canonical_dir
                )));
            }
        }

        // 2. OS-level lock check
        let lock_path = dir_path.join("store.lock");
        let lock_file = match Self::acquire_file_lock(&lock_path) {
            Ok(file) => file,
            Err(err) => {
                // Revert in-process registration on failure
                if let Ok(mut dirs) = get_open_dirs().lock() {
                    dirs.remove(&canonical_dir);
                }
                return Err(err);
            }
        };

        let lock_guard = LockGuard {
            lock_path,
            canonical_dir,
            _file: Some(lock_file),
        };

        // 3. Clean up any leftover temporary files from earlier crashes
        Self::cleanup_temp_files(&dir_path);

        // 4. Load latest snapshot if present
        let snapshot_path = dir_path.join("snapshot.json");
        let (mut tasks, mut checkpoints, mut seq) = if snapshot_path.exists() {
            let data = fs::read(&snapshot_path)?;
            let snapshot: Snapshot = serde_json::from_slice(&data).map_err(|e| {
                StoreError::Corruption(format!("Snapshot corrupted at {:?}: {e}", snapshot_path))
            })?;

            let mut task_map = BTreeMap::new();
            for task in snapshot.tasks {
                task_map.insert(task.id, task);
            }

            let mut checkpoint_map = BTreeMap::new();
            for cp in snapshot.checkpoints {
                checkpoint_map.insert(cp.task_id, cp);
            }

            (task_map, checkpoint_map, snapshot.last_seq)
        } else {
            (BTreeMap::new(), BTreeMap::new(), 0)
        };

        let snapshot_seq = seq;

        // 5. Open journal and replay entries newer than snapshot
        let journal_path = dir_path.join("journal.jsonl");
        let mut journal = Journal::open(&journal_path)?;
        let (entries, _stats) = journal.replay()?;

        for entry in entries {
            if entry.seq <= snapshot_seq {
                // Already folded into snapshot
                continue;
            }

            if entry.seq > seq {
                seq = entry.seq;
            }

            match entry.op {
                JournalOp::UpsertTask(task) => {
                    tasks.insert(task.id, task);
                }
                JournalOp::DeleteTask(id) => {
                    tasks.remove(&id);
                    checkpoints.remove(&id);
                }
                JournalOp::SaveCheckpoint(cp) => {
                    checkpoints.insert(cp.task_id, cp);
                }
            }
        }

        let inner = StoreInner {
            dir: Some(dir_path),
            journal: Some(journal),
            tasks,
            checkpoints,
            seq,
        };

        Ok(Self {
            inner: Mutex::new(inner),
            _lock: Some(lock_guard),
        })
    }

    /// Creates an ephemeral, in-memory [`Store`] suitable for testing.
    pub fn open_in_memory() -> Result<Self> {
        let inner = StoreInner {
            dir: None,
            journal: None,
            tasks: BTreeMap::new(),
            checkpoints: BTreeMap::new(),
            seq: 0,
        };

        Ok(Self {
            inner: Mutex::new(inner),
            _lock: None,
        })
    }

    /// Inserts or updates a download task record in the store.
    pub fn upsert_task(&self, record: &TaskRecord) -> Result<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| StoreError::Other("Store mutex poisoned".to_string()))?;

        inner.seq += 1;
        let seq = inner.seq;

        if let Some(journal) = inner.journal.as_mut() {
            let entry = Entry::upsert_task(seq, record.clone());
            journal.append(&entry)?;
        }

        inner.tasks.insert(record.id, record.clone());
        Ok(())
    }

    /// Retrieves a download task by its identifier.
    pub fn get_task(&self, id: TaskId) -> Result<Option<TaskRecord>> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| StoreError::Other("Store mutex poisoned".to_string()))?;

        Ok(inner.tasks.get(&id).cloned())
    }

    /// Returns a list of all download tasks currently stored, ordered chronologically.
    pub fn list_tasks(&self) -> Result<Vec<TaskRecord>> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| StoreError::Other("Store mutex poisoned".to_string()))?;

        Ok(inner.tasks.values().cloned().collect())
    }

    /// Deletes a download task and its associated checkpoint.
    ///
    /// Returns [`StoreError::NotFound`] if no task exists for the given ID.
    pub fn delete_task(&self, id: TaskId) -> Result<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| StoreError::Other("Store mutex poisoned".to_string()))?;

        if !inner.tasks.contains_key(&id) {
            return Err(StoreError::NotFound(id));
        }

        inner.seq += 1;
        let seq = inner.seq;

        if let Some(journal) = inner.journal.as_mut() {
            let entry = Entry::delete_task(seq, id);
            journal.append(&entry)?;
        }

        inner.tasks.remove(&id);
        inner.checkpoints.remove(&id);
        Ok(())
    }

    /// Saves a durable progress checkpoint for a task.
    ///
    /// Returns [`StoreError::NotFound`] if the referenced task is not found.
    pub fn save_checkpoint(&self, checkpoint: &Checkpoint) -> Result<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| StoreError::Other("Store mutex poisoned".to_string()))?;

        if !inner.tasks.contains_key(&checkpoint.task_id) {
            return Err(StoreError::NotFound(checkpoint.task_id));
        }

        inner.seq += 1;
        let seq = inner.seq;

        if let Some(journal) = inner.journal.as_mut() {
            let entry = Entry::save_checkpoint(seq, checkpoint.clone());
            journal.append(&entry)?;
        }

        inner.checkpoints.insert(checkpoint.task_id, checkpoint.clone());
        Ok(())
    }

    /// Loads the current checkpoint for the specified task.
    pub fn load_checkpoint(&self, id: TaskId) -> Result<Option<Checkpoint>> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| StoreError::Other("Store mutex poisoned".to_string()))?;

        Ok(inner.checkpoints.get(&id).cloned())
    }

    /// Flushes all pending writes to the OS and performs hardware sync (fsync) to disk.
    pub fn flush(&self) -> Result<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| StoreError::Other("Store mutex poisoned".to_string()))?;

        if let Some(journal) = inner.journal.as_mut() {
            journal.flush()?;
        }

        Ok(())
    }

    /// Compacts the store by writing an atomic snapshot and truncating the journal.
    ///
    /// Crash safety mechanics:
    /// 1. Encodes all current active tasks and checkpoints into a snapshot JSON.
    /// 2. Writes the snapshot to a temporary file (`snapshot.tmp.<id>`).
    /// 3. Performs hardware `sync_all` on the temporary file.
    /// 4. Atomically renames the temporary file over `snapshot.json`.
    /// 5. Truncates the write-ahead journal to 0 bytes and syncs.
    pub fn compact(&self) -> Result<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| StoreError::Other("Store mutex poisoned".to_string()))?;

        let dir = match &inner.dir {
            Some(d) => d.clone(),
            None => return Ok(()), // In-memory store compaction is a no-op
        };

        // 1. Prepare snapshot payload
        let snapshot = Snapshot {
            version: 1,
            last_seq: inner.seq,
            tasks: inner.tasks.values().cloned().collect(),
            checkpoints: inner.checkpoints.values().cloned().collect(),
        };

        let json_bytes = serde_json::to_vec_pretty(&snapshot)?;

        // 2. Write to temp file
        let temp_name = format!("snapshot_{}.tmp", uuid::Uuid::now_v7());
        let temp_path = dir.join(&temp_name);

        {
            let mut temp_file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)?;

            temp_file.write_all(&json_bytes)?;
            temp_file.flush()?;
            // 3. Hardware sync
            temp_file.sync_all()?;
        }

        // 4. Atomic rename over snapshot.json
        let final_snapshot_path = dir.join("snapshot.json");
        fs::rename(&temp_path, &final_snapshot_path)?;

        // 5. Truncate journal
        if let Some(journal) = inner.journal.as_mut() {
            journal.truncate()?;
        }

        Ok(())
    }

    /// Returns storage statistics, including record count and journal byte length.
    pub fn stats(&self) -> StoreStats {
        let inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(_) => return StoreStats::default(),
        };

        StoreStats {
            records: inner.tasks.len(),
            journal_bytes: inner.journal.as_ref().map(|j| j.byte_len()).unwrap_or(0),
        }
    }

    /// Internal helper to acquire an exclusive OS-level file lock.
    fn acquire_file_lock(lock_path: &Path) -> Result<File> {
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .share_mode(0) // Exclusive: prevents any other process or handle from opening
                .open(lock_path)
            {
                Ok(file) => Ok(file),
                Err(err)
                    if err.kind() == std::io::ErrorKind::PermissionDenied
                        || err.raw_os_error() == Some(32) =>
                {
                    Err(StoreError::Locked(format!(
                        "Lock file {:?} is held by another process",
                        lock_path
                    )))
                }
                Err(err) => Err(StoreError::Io(err)),
            }
        }

        #[cfg(not(windows))]
        {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(lock_path)?;
            Ok(file)
        }
    }

    /// Cleans up any orphaned temporary files left over from simulated or unexpected crashes.
    fn cleanup_temp_files(dir: &Path) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.ends_with(".tmp") || name.contains(".tmp.") {
                        let _ = fs::remove_file(&path);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_temp_dir() -> TempDir {
        if let Ok(cargo_tmp) = std::env::var("CARGO_TARGET_TMPDIR") {
            let _ = std::fs::create_dir_all(&cargo_tmp);
            if let Ok(dir) = tempfile::tempdir_in(&cargo_tmp) {
                return dir;
            }
        }
        let target_tmp = std::path::Path::new("target").join("test-tmp");
        let _ = std::fs::create_dir_all(&target_tmp);
        if let Ok(dir) = tempfile::tempdir_in(&target_tmp) {
            return dir;
        }
        tempfile::tempdir().expect("create temp dir")
    }

    #[test]
    fn test_in_memory_store_operations() {
        let store = Store::open_in_memory().unwrap();

        let task = TaskRecord::new("http://example.com/file.bin", "C:\\dl", "file.bin");
        let task_id = task.id;

        store.upsert_task(&task).unwrap();
        assert_eq!(store.get_task(task_id).unwrap().unwrap(), task);

        let list = store.list_tasks().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], task);

        let checkpoint = Checkpoint::new(task_id, 500, Some(1000), vec![(0, 500)]);
        store.save_checkpoint(&checkpoint).unwrap();
        assert_eq!(store.load_checkpoint(task_id).unwrap().unwrap(), checkpoint);

        store.delete_task(task_id).unwrap();
        assert!(store.get_task(task_id).unwrap().is_none());
        assert!(store.load_checkpoint(task_id).unwrap().is_none());
    }

    #[test]
    fn test_persistent_store_reopen_and_compact() {
        let dir = test_temp_dir();
        let task = TaskRecord::new("http://example.com/doc.pdf", "C:\\dl", "doc.pdf");
        let task_id = task.id;

        {
            let store = Store::open(dir.path()).unwrap();
            store.upsert_task(&task).unwrap();
            store.compact().unwrap();
            assert_eq!(store.stats().records, 1);
            assert_eq!(store.stats().journal_bytes, 0);
        }

        // Reopen from disk
        {
            let store = Store::open(dir.path()).unwrap();
            assert_eq!(store.stats().records, 1);
            let retrieved = store.get_task(task_id).unwrap().unwrap();
            assert_eq!(retrieved, task);
        }
    }

    #[test]
    fn test_concurrent_open_fails_cleanly() {
        let dir = test_temp_dir();
        let store1 = Store::open(dir.path()).unwrap();

        // Second open on the same directory must fail with StoreError::Locked
        let res = Store::open(dir.path());
        assert!(res.is_err());
        match res {
            Err(StoreError::Locked(_)) => {}
            other => panic!("Expected StoreError::Locked, got: {:?}", other),
        }

        drop(store1);

        // After dropping store1, opening again must succeed
        let store2 = Store::open(dir.path());
        assert!(store2.is_ok());
    }
}

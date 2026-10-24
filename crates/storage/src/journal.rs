//! Append-only JSON-Lines write-ahead journal.
//!
//! Provides [`Journal`], [`Entry`], [`JournalOp`], and [`ReplayStats`].
//! Handles append-only logging of mutations and tolerant replay where
//! half-written or truncated lines at the end of the file are safely ignored.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use sayl_core::id::TaskId;
use crate::error::{Result, StoreError};
use crate::record::{Checkpoint, TaskRecord};

/// Specific mutation operation recorded in the write-ahead journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum JournalOp {
    /// Task inserted or updated.
    UpsertTask(TaskRecord),
    /// Task deleted by identifier.
    DeleteTask(TaskId),
    /// Download checkpoint saved.
    SaveCheckpoint(Checkpoint),
}

/// A sequenced entry written to the append-only journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Monotonically increasing sequence number.
    pub seq: u64,
    /// Operation payload.
    pub op: JournalOp,
}

impl Entry {
    /// Constructs a new [`Entry`] with an upsert task operation.
    pub fn upsert_task(seq: u64, task: TaskRecord) -> Self {
        Self {
            seq,
            op: JournalOp::UpsertTask(task),
        }
    }

    /// Constructs a new [`Entry`] with a delete task operation.
    pub fn delete_task(seq: u64, id: TaskId) -> Self {
        Self {
            seq,
            op: JournalOp::DeleteTask(id),
        }
    }

    /// Constructs a new [`Entry`] with a save checkpoint operation.
    pub fn save_checkpoint(seq: u64, checkpoint: Checkpoint) -> Self {
        Self {
            seq,
            op: JournalOp::SaveCheckpoint(checkpoint),
        }
    }
}

/// Replay statistics returned after reading the journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReplayStats {
    /// Number of valid entries successfully parsed and replayed.
    pub entries_replayed: usize,
    /// Whether a partial, truncated line at the tail of the journal was ignored.
    pub truncated_tail_ignored: bool,
    /// Total bytes of valid, parsed journal content.
    pub valid_bytes: u64,
}

/// Append-only write-ahead journal backed by a JSON Lines file.
pub struct Journal {
    path: PathBuf,
    writer: Option<BufWriter<File>>,
    byte_len: u64,
}

impl Journal {
    /// Opens or creates a journal at the specified file path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)?;

        let byte_len = file.metadata()?.len();
        let writer = BufWriter::new(file);

        Ok(Self {
            path,
            writer: Some(writer),
            byte_len,
        })
    }

    /// Returns the current byte length of the journal on disk.
    pub fn byte_len(&self) -> u64 {
        self.byte_len
    }

    /// Returns the file path of this journal.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Appends a new entry to the journal file and flushes to the OS buffer.
    pub fn append(&mut self, entry: &Entry) -> Result<()> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| StoreError::Other("Journal writer not open".to_string()))?;

        let mut serialized = serde_json::to_vec(entry)?;
        serialized.push(b'\n');

        writer.write_all(&serialized)?;
        writer.flush()?;
        self.byte_len += serialized.len() as u64;

        Ok(())
    }

    /// Flushes user-space buffers and performs hardware sync (fsync) to disk.
    pub fn flush(&mut self) -> Result<()> {
        if let Some(writer) = self.writer.as_mut() {
            writer.flush()?;
            writer.get_ref().sync_all()?;
        }
        Ok(())
    }

    /// Reads and replays all entries from the journal file.
    ///
    /// If the journal ends with a partially written line (truncated tail),
    /// it is safely ignored and the journal file is trimmed back to the last
    /// valid byte offset. If a corruption occurs in the middle of the file,
    /// an error is returned.
    pub fn replay(&mut self) -> Result<(Vec<Entry>, ReplayStats)> {
        // Drop current writer to ensure no conflicting buffered data or locks
        self.writer = None;

        let (entries, stats) = Self::replay_path(&self.path)?;

        // Re-open file in read/write mode
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&self.path)?;

        if stats.truncated_tail_ignored {
            tracing::warn!(
                "Pruning truncated journal tail: keeping {} bytes of {}",
                stats.valid_bytes,
                file.metadata()?.len()
            );
            file.set_len(stats.valid_bytes)?;
            file.sync_all()?;
        }

        // Seek to the end of the file for subsequent appends
        file.seek(SeekFrom::End(0))?;
        self.byte_len = file.metadata()?.len();
        self.writer = Some(BufWriter::new(file));

        Ok((entries, stats))
    }

    /// Parses a journal file at `path` and extracts valid entries and replay statistics.
    pub fn replay_path(path: &Path) -> Result<(Vec<Entry>, ReplayStats)> {
        if !path.exists() {
            return Ok((Vec::new(), ReplayStats::default()));
        }

        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Self::replay_reader(reader)
    }

    /// Parses entries from any buffered reader, safely tolerating a truncated tail.
    pub fn replay_reader<R: BufRead>(mut reader: R) -> Result<(Vec<Entry>, ReplayStats)> {
        let mut lines = Vec::new();
        let mut raw_line = Vec::new();
        let mut offset = 0u64;

        loop {
            raw_line.clear();
            let bytes_read = reader.read_until(b'\n', &mut raw_line)?;
            if bytes_read == 0 {
                break;
            }

            let start_offset = offset;
            offset += bytes_read as u64;

            let line_bytes = raw_line.clone();
            lines.push((start_offset, offset, line_bytes));
        }

        let mut entries = Vec::new();
        let mut stats = ReplayStats::default();

        let num_lines = lines.len();
        for (i, (start_offset, end_offset, line_bytes)) in lines.into_iter().enumerate() {
            let line_str = match std::str::from_utf8(&line_bytes) {
                Ok(s) => s.trim(),
                Err(_) => {
                    // Check if all subsequent lines are empty
                    if i == num_lines - 1 {
                        stats.truncated_tail_ignored = true;
                        break;
                    } else {
                        return Err(StoreError::Corruption(format!(
                            "Invalid UTF-8 on journal line {i}"
                        )));
                    }
                }
            };

            if line_str.is_empty() {
                continue;
            }

            match serde_json::from_str::<Entry>(line_str) {
                Ok(entry) => {
                    entries.push(entry);
                    stats.entries_replayed += 1;
                    stats.valid_bytes = end_offset;
                }
                Err(err) => {
                    // If this is the last line or all subsequent non-empty lines are absent
                    if i == num_lines - 1 {
                        tracing::warn!(
                            "Ignoring truncated journal tail at offset {start_offset}: {err}"
                        );
                        stats.truncated_tail_ignored = true;
                        break;
                    } else {
                        return Err(StoreError::Corruption(format!(
                            "Corrupted journal line {i} (offset {start_offset}): {err}"
                        )));
                    }
                }
            }
        }

        Ok((entries, stats))
    }

    /// Atomically resets or truncates the journal file to 0 bytes.
    pub fn truncate(&mut self) -> Result<()> {
        if let Some(writer) = self.writer.as_mut() {
            writer.flush()?;
            let file = writer.get_mut();
            file.set_len(0)?;
            file.seek(SeekFrom::Start(0))?;
            file.sync_all()?;
        }
        self.byte_len = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
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
    fn test_journal_clean_append_and_replay() {
        let dir = test_temp_dir();
        let journal_path = dir.path().join("journal.jsonl");

        let mut journal = Journal::open(&journal_path).unwrap();
        let task = TaskRecord::new("http://example.com/a.zip", "C:\\dl", "a.zip");
        let task_id = task.id;

        let entry1 = Entry::upsert_task(1, task);
        let entry2 = Entry::delete_task(2, task_id);

        journal.append(&entry1).unwrap();
        journal.append(&entry2).unwrap();
        journal.flush().unwrap();

        assert!(journal.byte_len() > 0);

        let (replayed, stats) = journal.replay().unwrap();
        assert_eq!(stats.entries_replayed, 2);
        assert!(!stats.truncated_tail_ignored);
        assert_eq!(replayed.len(), 2);
        assert_eq!(replayed[0], entry1);
        assert_eq!(replayed[1], entry2);
    }

    #[test]
    fn test_journal_truncated_tail_ignored() {
        let dir = test_temp_dir();
        let journal_path = dir.path().join("journal.jsonl");

        let mut journal = Journal::open(&journal_path).unwrap();
        let task = TaskRecord::new("http://example.com/a.zip", "C:\\dl", "a.zip");
        let entry1 = Entry::upsert_task(1, task);
        journal.append(&entry1).unwrap();
        journal.flush().unwrap();

        // Deliberately append a half-written JSON line directly to the file
        {
            let mut file = OpenOptions::new().append(true).open(&journal_path).unwrap();
            file.write_all(b"{\"seq\":2,\"op\":{\"type\":\"UpsertTask\",\"payload\":{\"id\":").unwrap();
            file.flush().unwrap();
        }

        // Replay should succeed, ignore the partial tail, and return the 1 intact entry
        let (replayed, stats) = journal.replay().unwrap();
        assert_eq!(stats.entries_replayed, 1);
        assert!(stats.truncated_tail_ignored);
        assert_eq!(replayed.len(), 1);
        assert_eq!(replayed[0], entry1);

        // Appending a new entry after replay should work cleanly
        let task2 = TaskRecord::new("http://example.com/b.zip", "C:\\dl", "b.zip");
        let entry3 = Entry::upsert_task(3, task2);
        journal.append(&entry3).unwrap();
        journal.flush().unwrap();

        let (replayed2, stats2) = journal.replay().unwrap();
        assert_eq!(stats2.entries_replayed, 2);
        assert!(!stats2.truncated_tail_ignored);
        assert_eq!(replayed2.len(), 2);
        assert_eq!(replayed2[0], entry1);
        assert_eq!(replayed2[1], entry3);
    }

    #[test]
    fn test_journal_corrupt_middle_returns_error() {
        let dir = test_temp_dir();
        let journal_path = dir.path().join("journal.jsonl");

        let task1 = TaskRecord::new("http://example.com/a.zip", "C:\\dl", "a.zip");
        let task2 = TaskRecord::new("http://example.com/b.zip", "C:\\dl", "b.zip");
        let entry1 = Entry::upsert_task(1, task1);
        let entry2 = Entry::upsert_task(2, task2);

        let json1 = serde_json::to_string(&entry1).unwrap();
        let json2 = serde_json::to_string(&entry2).unwrap();

        // Write: valid line 1, corrupted line 2, valid line 3
        let content = format!("{json1}\nCORRUPTED_MIDDLE_LINE_JSON\n{json2}\n");
        std::fs::write(&journal_path, content).unwrap();

        let mut journal = Journal::open(&journal_path).unwrap();
        let res = journal.replay();
        assert!(res.is_err());
        match res {
            Err(StoreError::Corruption(msg)) => {
                assert!(msg.contains("Corrupted journal line"));
            }
            other => panic!("Expected Corruption error, got: {:?}", other),
        }
    }
}

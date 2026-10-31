//! Integration tests for sayl-storage.
//!
//! Validates crash safety, atomic snapshots, write-ahead journal replay,
//! corrupted tail tolerance, multi-range checkpoints, performance, and concurrency locks.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tempfile::TempDir;

use sayl_core::id::TaskId;
use sayl_core::state::DownloadState;
use sayl_storage::{Checkpoint, Store, StoreError, TaskRecord};

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
fn test_open_on_empty_dir_returns_empty_store() {
    let dir = test_temp_dir();
    let store = Store::open(dir.path()).expect("open empty store succeeds");

    let tasks = store.list_tasks().expect("list_tasks succeeds");
    assert!(tasks.is_empty(), "new store must have no tasks");

    let random_id = TaskId::new();
    let task = store.get_task(random_id).expect("get_task succeeds");
    assert!(task.is_none(), "nonexistent task should return None");

    let stats = store.stats();
    assert_eq!(stats.records, 0);
    assert_eq!(stats.journal_bytes, 0);
}

#[test]
fn test_reopen_after_writes_preserves_identical_records() {
    let dir = test_temp_dir();

    let mut task1 = TaskRecord::new("https://example.com/file1.bin", "C:\\dl", "file1.bin");
    task1.etag = Some("\"etag-1234\"".to_string());
    task1.written = 500;
    task1.total_size = Some(1000);
    task1.state = DownloadState::Downloading;

    let mut task2 = TaskRecord::new("https://example.com/file2.bin", "C:\\dl", "file2.bin");
    task2.last_modified = Some("Wed, 21 Oct 2025 07:28:00 GMT".to_string());
    task2.state = DownloadState::Completed;
    task2.written = 2048;
    task2.total_size = Some(2048);

    let id1 = task1.id;
    let id2 = task2.id;

    let cp1 = Checkpoint::new(id1, 500, Some(1000), vec![(0, 250), (250, 500)]);

    {
        let store = Store::open(dir.path()).expect("open store");
        store.upsert_task(&task1).expect("upsert task 1");
        store.upsert_task(&task2).expect("upsert task 2");
        store.save_checkpoint(&cp1).expect("save checkpoint 1");
        store.flush().expect("flush store");
    }

    // Reopen store from same directory
    {
        let store = Store::open(dir.path()).expect("reopen store");
        assert_eq!(store.stats().records, 2);

        let recovered1 = store
            .get_task(id1)
            .expect("get task 1")
            .expect("task 1 exists");
        let recovered2 = store
            .get_task(id2)
            .expect("get task 2")
            .expect("task 2 exists");

        assert_eq!(recovered1, task1);
        assert_eq!(recovered2, task2);

        let recovered_cp = store
            .load_checkpoint(id1)
            .expect("load cp")
            .expect("cp exists");
        assert_eq!(recovered_cp, cp1);

        let all = store.list_tasks().expect("list tasks");
        assert_eq!(all.len(), 2);
    }
}

#[test]
fn test_truncated_tail_recovery_ignores_corrupt_tail_and_recovers_intact_records() {
    let dir = test_temp_dir();

    let task1 = TaskRecord::new("https://example.com/a.iso", "C:\\dl", "a.iso");
    let task2 = TaskRecord::new("https://example.com/b.iso", "C:\\dl", "b.iso");
    let id1 = task1.id;
    let id2 = task2.id;

    {
        let store = Store::open(dir.path()).expect("open store");
        store.upsert_task(&task1).expect("upsert task 1");
        store.upsert_task(&task2).expect("upsert task 2");
        store.flush().expect("flush store");
    }

    // Simulate partial/truncated write at the tail of journal.jsonl
    let journal_path = dir.path().join("journal.jsonl");
    {
        let mut file = OpenOptions::new()
            .append(true)
            .open(&journal_path)
            .expect("open journal file for append");
        file.write_all(b"{\"seq\":3,\"op\":{\"type\":\"UpsertTask\",\"payload\":{\"id\":\"broken")
            .expect("write truncated tail");
        file.flush().expect("flush truncated tail");
    }

    // Reopen store — corrupted tail must be safely ignored, intact records recovered
    {
        let store = Store::open(dir.path()).expect("reopen store with truncated tail succeeds");
        assert_eq!(store.stats().records, 2);

        let r1 = store
            .get_task(id1)
            .expect("get task 1")
            .expect("task 1 intact");
        let r2 = store
            .get_task(id2)
            .expect("get task 2")
            .expect("task 2 intact");
        assert_eq!(r1, task1);
        assert_eq!(r2, task2);

        // Subsequent writes after recovery must work seamlessly
        let task3 = TaskRecord::new("https://example.com/c.iso", "C:\\dl", "c.iso");
        let id3 = task3.id;
        store
            .upsert_task(&task3)
            .expect("upsert task 3 after recovery");
        assert_eq!(store.stats().records, 3);
        assert_eq!(store.get_task(id3).expect("get task 3").unwrap(), task3);
    }
}

#[test]
fn test_simulate_crash_with_snapshot_and_partial_journal() {
    let dir = test_temp_dir();

    let task1 = TaskRecord::new("https://example.com/1.tar", "C:\\dl", "1.tar");
    let task2 = TaskRecord::new("https://example.com/2.tar", "C:\\dl", "2.tar");
    let task3 = TaskRecord::new("https://example.com/3.tar", "C:\\dl", "3.tar");
    let task4 = TaskRecord::new("https://example.com/4.tar", "C:\\dl", "4.tar");

    let id1 = task1.id;
    let id2 = task2.id;
    let id3 = task3.id;
    let id4 = task4.id;

    // Step 1: Write tasks 1, 2, 3 and compact into an atomic snapshot
    {
        let store = Store::open(dir.path()).expect("open store");
        store.upsert_task(&task1).expect("upsert 1");
        store.upsert_task(&task2).expect("upsert 2");
        store.upsert_task(&task3).expect("upsert 3");
        store.compact().expect("compact snapshot");
        assert_eq!(store.stats().records, 3);
        assert_eq!(store.stats().journal_bytes, 0);

        // Step 2: Append task 4 to journal after snapshot
        store.upsert_task(&task4).expect("upsert 4");
        store.flush().expect("flush journal");
    }

    // Step 3: Simulate crash mid-write of task 5 directly into journal.jsonl
    let journal_path = dir.path().join("journal.jsonl");
    {
        let mut file = OpenOptions::new()
            .append(true)
            .open(&journal_path)
            .expect("open journal for append");
        file.write_all(b"{\"seq\":5,\"op\":{\"type\":\"UpsertTask\",\"payload\":{\"id\":")
            .expect("write partial line");
        file.flush().expect("flush");
    }

    // Step 4: Reopen — recovery must return the last consistent state (tasks 1, 2, 3 from snapshot + 4 from journal)
    {
        let store = Store::open(dir.path()).expect("recovery open succeeds");
        assert_eq!(store.stats().records, 4);

        assert_eq!(store.get_task(id1).unwrap().unwrap(), task1);
        assert_eq!(store.get_task(id2).unwrap().unwrap(), task2);
        assert_eq!(store.get_task(id3).unwrap().unwrap(), task3);
        assert_eq!(store.get_task(id4).unwrap().unwrap(), task4);
    }
}

#[test]
fn test_delete_then_compact_then_reopen_record_really_gone() {
    let dir = test_temp_dir();

    let task1 = TaskRecord::new("https://example.com/keep.txt", "C:\\dl", "keep.txt");
    let task2 = TaskRecord::new("https://example.com/del.txt", "C:\\dl", "del.txt");
    let id1 = task1.id;
    let id2 = task2.id;

    {
        let store = Store::open(dir.path()).expect("open store");
        store.upsert_task(&task1).expect("upsert 1");
        store.upsert_task(&task2).expect("upsert 2");
        assert_eq!(store.stats().records, 2);

        // Delete task 2
        store.delete_task(id2).expect("delete task 2");
        assert_eq!(store.stats().records, 1);
        assert!(store.get_task(id2).expect("get deleted").is_none());

        // Compact folds delete into snapshot and truncates journal
        store.compact().expect("compact");
        assert_eq!(store.stats().journal_bytes, 0);
    }

    // Reopen store from disk
    {
        let store = Store::open(dir.path()).expect("reopen store");
        assert_eq!(store.stats().records, 1);
        assert_eq!(store.stats().journal_bytes, 0);

        // Record must be truly gone
        assert!(store.get_task(id2).expect("check deleted").is_none());

        // Survived record is intact
        let retrieved = store.get_task(id1).expect("get kept").expect("kept exists");
        assert_eq!(retrieved, task1);
    }
}

#[test]
fn test_multi_range_checkpoint_roundtrips_exactly() {
    let dir = test_temp_dir();
    let store = Store::open(dir.path()).expect("open store");

    let task = TaskRecord::new(
        "https://example.com/large.iso",
        PathBuf::from("C:\\dl"),
        "large.iso",
    );
    let task_id = task.id;
    store.upsert_task(&task).expect("upsert task");

    let ranges = vec![
        (0, 1024),
        (2048, 4096),
        (8192, 16384),
        (32768, 65536),
        (131072, 262144),
        (524288, 1048576),
    ];
    let checkpoint = Checkpoint::new(task_id, 1048576, Some(2097152), ranges.clone());

    store.save_checkpoint(&checkpoint).expect("save checkpoint");

    // Load from active store
    let loaded = store
        .load_checkpoint(task_id)
        .expect("load cp")
        .expect("cp exists");
    assert_eq!(loaded, checkpoint);
    assert_eq!(loaded.range_map, ranges);

    // Compact and reopen
    store.compact().expect("compact store");
    drop(store);

    let reopened = Store::open(dir.path()).expect("reopen store");
    let loaded_reopened = reopened
        .load_checkpoint(task_id)
        .expect("load cp")
        .expect("cp exists");
    assert_eq!(loaded_reopened, checkpoint);
    assert_eq!(loaded_reopened.range_map, ranges);
}

#[test]
fn test_ten_thousand_upserts_and_query_correctness_under_five_seconds() {
    let dir = test_temp_dir();
    let store = Store::open(dir.path()).expect("open store");

    let count = 10_000;
    let mut sampled_ids = Vec::new();

    let start = Instant::now();

    for i in 0..count {
        let filename = format!("file_{i}.bin");
        let task = TaskRecord::new("https://example.com/batch", "C:\\dl", filename);
        if i % 2000 == 0 || i == count - 1 {
            sampled_ids.push(task.id);
        }
        store.upsert_task(&task).expect("upsert in loop");
    }

    store.flush().expect("flush store after 10,000 upserts");

    let list = store.list_tasks().expect("list_tasks");
    assert_eq!(list.len(), count, "must hold all 10,000 tasks");

    // Verify sample correctness
    for sampled_id in sampled_ids {
        let task = store
            .get_task(sampled_id)
            .expect("get_task")
            .expect("sampled task exists");
        assert_eq!(task.id, sampled_id);
    }

    let elapsed = start.elapsed();
    println!("10,000 upserts + list + queries completed in {:?}", elapsed);
    assert!(
        elapsed < Duration::from_secs(5),
        "10,000 upserts must complete well under 5 seconds (took {:?})",
        elapsed
    );
}

#[test]
fn test_concurrency_second_open_fails_cleanly() {
    let dir = test_temp_dir();

    // First open succeeds and holds directory lock
    let store1 = Store::open(dir.path()).expect("first open succeeds");

    let task = TaskRecord::new("https://example.com/lock.bin", "C:\\dl", "lock.bin");
    store1.upsert_task(&task).expect("upsert in store 1");

    // Second open on the exact same directory must fail cleanly with StoreError::Locked
    let open2_result = Store::open(dir.path());
    assert!(open2_result.is_err(), "second open must fail");
    match open2_result {
        Err(StoreError::Locked(msg)) => {
            assert!(msg.contains("already open") || msg.contains("locked"));
        }
        other => panic!("Expected StoreError::Locked, got: {:?}", other),
    }

    // Verify first store continues to function correctly without corruption
    assert_eq!(store1.stats().records, 1);

    // Dropping first store releases lock
    drop(store1);

    // Third open must now succeed
    let store3 = Store::open(dir.path()).expect("third open after drop succeeds");
    assert_eq!(store3.stats().records, 1);
    assert_eq!(store3.get_task(task.id).unwrap().unwrap(), task);
}

use std::collections::HashSet;
use std::fs;
use std::time::{Duration, Instant};

use sayl_app::{Engine, EngineConfig};
use sayl_core::state::DownloadState;
use sayl_storage::{Checkpoint, Store};
use sayl_test_http::{generate_bytes, HttpFixtureServer, Scenario};
use tempfile::TempDir;

fn make_temp_dir() -> TempDir {
    if let Ok(cargo_tmp) = std::env::var("CARGO_TARGET_TMPDIR") {
        let _ = fs::create_dir_all(&cargo_tmp);
        if let Ok(dir) = tempfile::tempdir_in(&cargo_tmp) {
            return dir;
        }
    }
    let target_tmp = std::path::Path::new("target").join("test-tmp");
    let _ = fs::create_dir_all(&target_tmp);
    if let Ok(dir) = tempfile::tempdir_in(&target_tmp) {
        return dir;
    }
    tempfile::tempdir().expect("create temp dir")
}

fn make_temp_dirs() -> (TempDir, TempDir) {
    (make_temp_dir(), make_temp_dir())
}

/// Helper to extract all Range headers for a specific path fragment.
async fn get_range_headers(server: &HttpFixtureServer, path_fragment: &str) -> Vec<String> {
    let recorded = server.recorded_requests().await;
    recorded
        .iter()
        .filter(|req| {
            req.method == "GET" && req.path.contains(path_fragment) && req.range_header.is_some()
        })
        .map(|req| req.range_header.clone().unwrap())
        .collect()
}

// ============================================================================
// Scenario 1: Sequential crash mid-transfer
// ============================================================================
#[tokio::test]
async fn test_recovery_sequential_crash_mid_transfer() {
    let (dl_dir, store_dir) = make_temp_dirs();
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let size = 16 * 1024 * 1024; // 16 MiB
    server.set_resource_size(size);
    server.set_delay(Duration::from_millis(50)); // Slower to allow crash mid-transfer
    let addr = server.start().await.expect("start fixture server");

    let config = EngineConfig {
        max_connections: 1,
        ..Default::default()
    };

    let task_id;
    let part_path;
    {
        let engine =
            Engine::new(dl_dir.path(), store_dir.path(), config.clone()).expect("create engine");
        let url = format!("http://{addr}/test_seq_crash.bin");
        task_id = engine.add(&url).await.expect("add task");
        part_path = dl_dir.path().join("test_seq_crash.bin.part");

        // Run download in background and wait for state to become Downloading
        let handle = engine.spawn_start(task_id);

        // Poll until task state is Downloading (past probe phase)
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            let task = engine.get_task(task_id).unwrap().unwrap();
            if task.state == DownloadState::Downloading {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let task = engine.get_task(task_id).unwrap().unwrap();
        assert_eq!(
            task.state,
            DownloadState::Downloading,
            "task should be Downloading before crash"
        );

        // Give it time to write some data but not complete
        tokio::time::sleep(Duration::from_millis(1000)).await;

        // Abort the background task to simulate crash (engine drop alone doesn't stop it)
        handle.abort();
        drop(engine);
        let _ = handle.await; // Clean up
    }

    // Verify .part file exists and has some data
    assert!(part_path.exists(), ".part file must exist after crash");
    let part_len = fs::metadata(&part_path).unwrap().len();
    assert!(
        part_len > 0 && part_len < size as u64,
        "partial file must have some but not all data: got {} bytes",
        part_len
    );
    let checkpoint_offset = part_len;

    // Reopen engine and recover
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("reopen engine");
    let _recovered = engine.recover().await.expect("recover must not panic");
    // recover() returns tasks whose state changed; Queued tasks are re-enqueued but not in list
    // Check task state instead
    let recovered_task = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(
        recovered_task.state,
        DownloadState::Paused,
        "recovered task should be Paused, was {:?}",
        recovered_task.state
    );

    // Start the task again - it should resume
    engine.start(task_id).await.expect("resume download");

    // Verify completion
    let completed_task = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(completed_task.state, DownloadState::Completed);
    assert_eq!(completed_task.written, size as u64);

    // Verify final file is byte-identical
    let final_path = dl_dir.path().join(&completed_task.filename);
    let bytes = tokio::fs::read(&final_path).await.expect("read final file");
    assert_eq!(
        bytes,
        generate_bytes(size),
        "final file must be byte-identical to fixture"
    );

    // Assert second run used Range starting at checkpoint offset
    let ranges = get_range_headers(&server, "test_seq_crash").await;
    let resume_range = ranges
        .iter()
        .find(|r| r.starts_with("bytes=") && !r.contains("0-0"))
        .expect("must have a resume range request");
    let resume_start = resume_range
        .strip_prefix("bytes=")
        .unwrap()
        .split('-')
        .next()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    assert_eq!(
        resume_start, checkpoint_offset,
        "resume must start at checkpoint offset {}, got {}",
        checkpoint_offset, resume_start
    );
}

// ============================================================================
// Scenario 2: Parallel download completes with ownership discipline
// ============================================================================
#[tokio::test]
async fn test_recovery_parallel_download_completes_once() {
    let (dl_dir, store_dir) = make_temp_dirs();
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let size = 4 * 1024 * 1024; // 4 MiB
    server.set_resource_size(size);
    server.set_delay(Duration::from_millis(20));
    let addr = server.start().await.expect("start fixture server");

    let config = EngineConfig {
        max_connections: 4,
        chunk_size: 1024 * 1024, // 4 parallel ranges
        ..Default::default()
    };

    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");
    let url = format!("http://{addr}/test_parallel_complete.bin");
    let task_id = engine.add(&url).await.expect("add task");
    let filename = engine.get_task(task_id).unwrap().unwrap().filename.clone();

    engine.spawn_start(task_id);

    // Wait to terminal.
    let completed = {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let task = engine.get_task(task_id).unwrap().unwrap();
            if task.state.is_terminal() {
                break task;
            }
            assert!(
                Instant::now() < deadline,
                "parallel download did not finish"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };
    assert_eq!(completed.state, DownloadState::Completed);
    assert_eq!(completed.written, size as u64);

    // Final file must be byte-identical.
    let final_path = dl_dir.path().join(&completed.filename);
    let bytes = tokio::fs::read(&final_path).await.expect("read final file");
    assert_eq!(
        bytes,
        generate_bytes(size),
        "final file must be byte-identical for parallel download"
    );

    // Ownership discipline: every byte must be fetched exactly once. The sum of
    // all range-request lengths must equal `size` — no range re-requested, no
    // range requested twice, no false complete leaving bytes missing.
    // Exclude the initial 1-byte probe (bytes=0-0) which the fixture issues per GET.
    let ranges: Vec<_> = get_range_headers(&server, &filename)
        .await
        .into_iter()
        .filter(|r| !r.contains("0-0"))
        .collect();
    let mut fetched: u64 = 0;
    let mut ranges_seen: HashSet<(u64, u64)> = HashSet::new();
    for r in &ranges {
        let body = r.strip_prefix("bytes=").unwrap_or(r);
        let mut parts = body.split('-');
        let start: u64 = parts.next().unwrap().parse().unwrap();
        let end: u64 = parts.next().unwrap().parse().unwrap();
        let len = end - start + 1;
        fetched += len;
        assert!(
            ranges_seen.insert((start, end)),
            "range {r} was requested more than once"
        );
    }
    assert_eq!(
        fetched, size as u64,
        "total bytes fetched ({fetched}) must equal size ({size}) — no wasted or missing ranges"
    );
}

// ============================================================================
// Scenario 3: Crash after all bytes written but before finalisation
// ============================================================================
#[tokio::test]
async fn test_recovery_crash_after_write_before_finalise() {
    let (dl_dir, store_dir) = make_temp_dirs();
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let size = 2 * 1024 * 1024; // 2 MiB
    server.set_resource_size(size);
    let addr = server.start().await.expect("start fixture server");

    let config = EngineConfig {
        max_connections: 1,
        ..Default::default()
    };

    let task_id;
    {
        let engine =
            Engine::new(dl_dir.path(), store_dir.path(), config.clone()).expect("create engine");
        let url = format!("http://{addr}/test_before_finalise.bin");
        task_id = engine.add(&url).await.expect("add task");
        engine.start(task_id).await.expect("start download");
        // Download completed, engine still alive
        drop(engine);
    }

    // Now manually set the task state to Downloading (non-terminal) but file is complete
    {
        let store = Store::open(store_dir.path()).expect("open store");
        let mut task = store.get_task(task_id).unwrap().unwrap();
        assert_eq!(task.state, DownloadState::Completed);
        // Simulate crash during finalisation: revert state to Downloading
        task.state = DownloadState::Downloading;
        store.upsert_task(&task).expect("upsert corrupted state");
    }

    // Verify .part file has full content (or final file exists)
    let part_path = dl_dir.path().join("test_before_finalise.bin.part");
    let final_path = dl_dir.path().join("test_before_finalise.bin");
    let has_full_part =
        part_path.exists() && fs::metadata(&part_path).unwrap().len() == size as u64;
    let has_final = final_path.exists() && fs::metadata(&final_path).unwrap().len() == size as u64;
    assert!(
        has_full_part || has_final,
        "file must have all bytes somewhere"
    );

    // Reopen and recover
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("reopen engine");
    let recovered = engine.recover().await.expect("recover must not panic");
    assert!(recovered.contains(&task_id));

    // Start should finalise without re-downloading
    engine.start(task_id).await.expect("finalize download");

    let completed_task = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(completed_task.state, DownloadState::Completed);
    assert_eq!(completed_task.written, size as u64);

    // Verify no additional download occurred (only the initial probe + maybe one range if needed)
    let ranges = get_range_headers(&server, "test_before_finalise").await;
    let download_ranges: Vec<_> = ranges.iter().filter(|r| !r.contains("0-0")).collect();
    assert!(
        download_ranges.is_empty(),
        "no re-download should occur, but got ranges: {:?}",
        download_ranges
    );

    // Final file must exist and be correct
    let final_path = dl_dir.path().join(&completed_task.filename);
    assert!(final_path.exists(), "final file must exist");
    let bytes = tokio::fs::read(&final_path).await.expect("read final file");
    assert_eq!(
        bytes,
        generate_bytes(size),
        "final file must be byte-identical"
    );
}

// ============================================================================
// Scenario 4: Truncated or corrupt checkpoint
// ============================================================================
#[tokio::test]
async fn test_recovery_corrupt_checkpoint() {
    let (dl_dir, store_dir) = make_temp_dirs();
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let size = 16 * 1024 * 1024; // 16 MiB
    server.set_resource_size(size);
    server.set_delay(Duration::from_millis(50));
    let addr = server.start().await.expect("start fixture server");

    let config = EngineConfig {
        max_connections: 1,
        ..Default::default()
    };

    let task_id;
    let part_path;
    {
        // First, create a task and start it so it gets a Downloading state and a .part file
        let engine =
            Engine::new(dl_dir.path(), store_dir.path(), config.clone()).expect("create engine");
        let url = format!("http://{addr}/test_corrupt_cp.bin");
        task_id = engine.add(&url).await.expect("add task");
        part_path = dl_dir.path().join("test_corrupt_cp.bin.part");
        let handle = engine.spawn_start(task_id);

        // Wait for state to become Downloading
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            let task = engine.get_task(task_id).unwrap().unwrap();
            if task.state == DownloadState::Downloading {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let task = engine.get_task(task_id).unwrap().unwrap();
        assert_eq!(task.state, DownloadState::Downloading);

        // Give it time to write some data but not complete
        tokio::time::sleep(Duration::from_millis(1000)).await;

        // Abort to simulate crash
        handle.abort();
        drop(engine);
        let _ = handle.await;
    }

    // Verify .part file exists (download was in progress)
    assert!(part_path.exists(), ".part file must exist");
    let part_len = fs::metadata(&part_path).unwrap().len();
    assert!(
        part_len > 0 && part_len < size as u64,
        "partial file must have some data"
    );

    // Now corrupt the checkpoint
    {
        let store = Store::open(store_dir.path()).expect("open store");
        let garbage_checkpoint = Checkpoint::new(
            task_id,
            999999, // bogus written
            Some(size as u64),
            vec![(0, 999999)], // bogus range
        );
        store
            .save_checkpoint(&garbage_checkpoint)
            .expect("save garbage checkpoint");
    }

    // Reopen and recover - should not panic, should restart task
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("reopen engine");
    let _recovered = engine
        .recover()
        .await
        .expect("recover must not panic on corrupt checkpoint");
    let recovered_task = engine.get_task(task_id).unwrap().unwrap();
    // Task should be in a recoverable state (Paused/Queued/Failed)
    assert!(
        recovered_task.state == DownloadState::Paused
            || recovered_task.state == DownloadState::Queued
            || recovered_task.state == DownloadState::Failed,
        "recovered task must be in Paused/Queued/Failed, was {:?}",
        recovered_task.state
    );

    // Start the task - it should complete correctly from zero
    engine
        .start(task_id)
        .await
        .expect("start download after corrupt checkpoint");

    let completed_task = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(completed_task.state, DownloadState::Completed);
    assert_eq!(completed_task.written, size as u64);

    // Verify final file is correct
    let final_path = dl_dir.path().join(&completed_task.filename);
    let bytes = tokio::fs::read(&final_path).await.expect("read final file");
    assert_eq!(
        bytes,
        generate_bytes(size),
        "final file must be byte-identical despite corrupt checkpoint"
    );
}

// ============================================================================
// Scenario 5: Terminal tasks are untouched
// ============================================================================
#[tokio::test]
async fn test_recovery_terminal_tasks_untouched() {
    let (dl_dir, store_dir) = make_temp_dirs();
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let size = 512 * 1024; // 512 KiB
    server.set_resource_size(size);
    let addr = server.start().await.expect("start fixture server");

    let config = EngineConfig {
        max_connections: 1,
        ..Default::default()
    };

    let (completed_task_id, failed_task_id, cancelled_task_id);

    // Phase 1: Create completed task
    {
        let engine =
            Engine::new(dl_dir.path(), store_dir.path(), config.clone()).expect("create engine");
        let url1 = format!("http://{addr}/terminal_completed.bin");
        completed_task_id = engine.add(&url1).await.expect("add task1");
        engine.start(completed_task_id).await.expect("start task1");
        let t1 = engine.get_task(completed_task_id).unwrap().unwrap();
        assert_eq!(t1.state, DownloadState::Completed);
        drop(engine);
    }

    // Phase 2: Create failed and cancelled tasks (separate engine instances to avoid lock conflicts)
    {
        let engine =
            Engine::new(dl_dir.path(), store_dir.path(), config.clone()).expect("create engine");
        let url2 = format!("http://{addr}/terminal_failed.bin");
        failed_task_id = engine.add(&url2).await.expect("add task2");
        // Manually mark as failed via store (engine dropped first)
        drop(engine);
    }
    {
        let store = Store::open(store_dir.path()).expect("open store for failed task");
        let mut task = store.get_task(failed_task_id).unwrap().unwrap();
        task.state = DownloadState::Failed;
        task.error = Some("simulated failure".into());
        store.upsert_task(&task).unwrap();
    }

    {
        let engine =
            Engine::new(dl_dir.path(), store_dir.path(), config.clone()).expect("create engine");
        let url3 = format!("http://{addr}/terminal_cancelled.bin");
        cancelled_task_id = engine.add(&url3).await.expect("add task3");
        drop(engine);
    }
    {
        let store = Store::open(store_dir.path()).expect("open store for cancelled task");
        let mut task = store.get_task(cancelled_task_id).unwrap().unwrap();
        task.state = DownloadState::Cancelled;
        store.upsert_task(&task).unwrap();
    }

    // Verify files before recovery
    let t1_final = dl_dir.path().join("terminal_completed.bin");
    let t1_bytes_before = tokio::fs::read(&t1_final).await.unwrap();

    // Reopen and recover
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("reopen engine");
    let recovered = engine.recover().await.expect("recover must not panic");

    // Terminal tasks should NOT be in recovered list
    assert!(
        !recovered.contains(&completed_task_id),
        "Completed task must not be in recovered list"
    );
    assert!(
        !recovered.contains(&failed_task_id),
        "Failed task must not be in recovered list"
    );
    assert!(
        !recovered.contains(&cancelled_task_id),
        "Cancelled task must not be in recovered list"
    );

    // Check states unchanged
    let t1 = engine.get_task(completed_task_id).unwrap().unwrap();
    assert_eq!(t1.state, DownloadState::Completed);
    let t2 = engine.get_task(failed_task_id).unwrap().unwrap();
    assert_eq!(t2.state, DownloadState::Failed);
    let t3 = engine.get_task(cancelled_task_id).unwrap().unwrap();
    assert_eq!(t3.state, DownloadState::Cancelled);

    // Completed task's file must not be modified
    let t1_bytes_after = tokio::fs::read(&t1_final).await.unwrap();
    assert_eq!(
        t1_bytes_after, t1_bytes_before,
        "Completed task file must not be modified"
    );
    assert_eq!(
        t1_bytes_after,
        generate_bytes(size),
        "Completed task file must remain byte-correct"
    );

    // No new downloads should have been triggered for terminal tasks
    let all_requests = server.recorded_requests().await;
    let download_requests: Vec<_> = all_requests
        .iter()
        .filter(|r| {
            r.method == "GET"
                && r.range_header.is_some()
                && !r.range_header.as_ref().unwrap().contains("0-0")
        })
        .collect();
    assert!(
        download_requests.is_empty(),
        "no download requests should occur for terminal tasks on recovery"
    );
}

// ============================================================================
// Scenario 6: Partial file shorter than recorded offset
// ============================================================================
#[tokio::test]
async fn test_recovery_partial_file_shorter_than_checkpoint() {
    let (dl_dir, store_dir) = make_temp_dirs();
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let size = 16 * 1024 * 1024; // 16 MiB
    server.set_resource_size(size);
    server.set_delay(Duration::from_millis(50));
    let addr = server.start().await.expect("start fixture server");

    let config = EngineConfig {
        max_connections: 1,
        ..Default::default()
    };

    let task_id;
    let part_path;
    {
        let engine =
            Engine::new(dl_dir.path(), store_dir.path(), config.clone()).expect("create engine");
        let url = format!("http://{addr}/test_truncated_part.bin");
        task_id = engine.add(&url).await.expect("add task");
        part_path = dl_dir.path().join("test_truncated_part.bin.part");

        let handle = engine.spawn_start(task_id);

        // Poll until task state is Downloading (past probe phase)
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            let task = engine.get_task(task_id).unwrap().unwrap();
            if task.state == DownloadState::Downloading {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let task = engine.get_task(task_id).unwrap().unwrap();
        assert_eq!(
            task.state,
            DownloadState::Downloading,
            "task should be Downloading before crash"
        );

        // Give it more time to write data
        tokio::time::sleep(Duration::from_millis(1000)).await;

        // Abort to simulate crash
        handle.abort();
        drop(engine);
        let _ = handle.await;
    }

    // Shrink the .part file behind engine's back
    assert!(part_path.exists());
    let original_len = fs::metadata(&part_path).unwrap().len();
    assert!(original_len > 0);
    let truncated_len = original_len / 2;
    let file = fs::OpenOptions::new().write(true).open(&part_path).unwrap();
    file.set_len(truncated_len).unwrap();
    drop(file);
    let new_len = fs::metadata(&part_path).unwrap().len();
    assert_eq!(new_len, truncated_len, "file must be truncated");

    // Also update checkpoint to match original (simulating stale checkpoint)
    {
        let store = Store::open(store_dir.path()).expect("open store");
        let task = store.get_task(task_id).unwrap().unwrap();
        // Keep task.written at original (stale) value
        let checkpoint = Checkpoint::new(
            task_id,
            task.written,
            Some(size as u64),
            vec![(0, task.written)],
        );
        store.save_checkpoint(&checkpoint).unwrap();
    }

    // Reopen and recover
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("reopen engine");
    let _recovered = engine.recover().await.expect("recover must not panic");
    let recovered_task = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(
        recovered_task.state,
        DownloadState::Paused,
        "recovered task should be Paused, was {:?}",
        recovered_task.state
    );

    // Start - should reconcile and complete correctly
    engine
        .start(task_id)
        .await
        .expect("resume download after truncation");

    let completed_task = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(completed_task.state, DownloadState::Completed);
    assert_eq!(completed_task.written, size as u64);

    // Final file must be correct
    let final_path = dl_dir.path().join(&completed_task.filename);
    let bytes = tokio::fs::read(&final_path).await.expect("read final file");
    assert_eq!(
        bytes,
        generate_bytes(size),
        "final file must be byte-identical despite truncation"
    );

    // Should have reconciled to actual on-disk length (truncated_len)
    // and resumed from there (not from 0 and not from stale checkpoint)
    let ranges = get_range_headers(&server, "test_truncated_part").await;
    let download_ranges: Vec<_> = ranges.iter().filter(|r| !r.contains("0-0")).collect();
    if !download_ranges.is_empty() {
        let first = download_ranges[0];
        let start = first
            .strip_prefix("bytes=")
            .unwrap()
            .split('-')
            .next()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        // Resume offset must equal actual file length at recovery time
        assert_eq!(
            start, truncated_len,
            "resume must start at actual file length {}, got start={}",
            truncated_len, start
        );
    }

    // Silence the unused-import warning the original template carried.
    let _ = HashSet::<u64>::new();
}

// ============================================================================
// Scenario: parallel crash with preallocated .part and no valid checkpoint
// must NOT be treated as complete (holes would be counted as downloaded).
// ============================================================================
#[tokio::test]
async fn test_recovery_parallel_preallocated_part_without_checkpoint_not_complete() {
    let (dl_dir, store_dir) = make_temp_dirs();
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let size = 16 * 1024 * 1024; // 16 MiB
    server.set_resource_size(size);
    server.set_delay(Duration::from_millis(50));
    let addr = server.start().await.expect("start fixture server");

    let config = EngineConfig {
        max_connections: 4,
        ..Default::default()
    };

    let task_id;
    let part_path;
    {
        let engine =
            Engine::new(dl_dir.path(), store_dir.path(), config.clone()).expect("create engine");
        let url = format!("http://{addr}/test_par_prealloc.bin");
        task_id = engine.add(&url).await.expect("add task");
        part_path = dl_dir.path().join("test_par_prealloc.bin.part");
        let handle = engine.spawn_start(task_id);

        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            let task = engine.get_task(task_id).unwrap().unwrap();
            if task.state == DownloadState::Downloading {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Abort to simulate crash
        handle.abort();
        drop(engine);
        let _ = handle.await;
    }

    // Make the parallel-style precondition explicit: .part preallocated to total_size.
    {
        let f = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&part_path)
            .expect("open part file");
        f.set_len(size as u64).expect("preallocate part file");
    }

    // No valid checkpoint: write a corrupt one so recovery discards it.
    {
        // Detached parallel workers may still hold the store briefly after abort.
        let deadline = Instant::now() + Duration::from_secs(10);
        let store = loop {
            match Store::open(store_dir.path()) {
                Ok(s) => break s,
                Err(e) if Instant::now() < deadline => {
                    let _ = e;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(e) => panic!("open store: {e:?}"),
            }
        };
        let garbage = Checkpoint::new(task_id, u64::MAX / 2, Some(size as u64), vec![(0, 1)]);
        store
            .save_checkpoint(&garbage)
            .expect("save garbage checkpoint");
    }

    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("reopen engine");
    let _ = engine.recover().await.expect("recover must not panic");
    let recovered = engine.get_task(task_id).unwrap().unwrap();
    assert_ne!(
        recovered.state,
        DownloadState::Completed,
        "preallocated part without checkpoint must not be marked complete"
    );
    assert!(
        recovered.written < size as u64,
        "recovery must not trust a preallocated part length as written bytes: {}",
        recovered.written
    );

    engine.start(task_id).await.expect("resume download");
    let done = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(done.state, DownloadState::Completed);
    let final_path = dl_dir.path().join(&done.filename);
    let bytes = tokio::fs::read(&final_path).await.expect("read final file");
    assert_eq!(
        bytes,
        generate_bytes(size),
        "final file must be byte-identical after recovery"
    );
}

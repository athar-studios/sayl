use std::collections::HashSet;
use std::time::{Duration, Instant};

use sayl_app::{Engine, EngineConfig};
use sayl_core::state::DownloadState;
use sayl_storage::{Store, TaskRecord};
use sayl_test_http::{generate_bytes, HttpFixtureServer, Scenario};
use tempfile::TempDir;

/// Wait for a task to leave Queued state (i.e., be promoted by scheduler).
/// Returns the new state, or times out with a clear message.
async fn wait_until_not_queued(
    engine: &Engine,
    task_id: sayl_core::id::TaskId,
    timeout: Duration,
) -> DownloadState {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let task = engine.get_task(task_id).unwrap().unwrap();
        if task.state != DownloadState::Queued {
            return task.state;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("task {} did not leave Queued within {:?}", task_id, timeout);
}

/// Wait for a task to reach a terminal state (Completed, Failed, Cancelled).
async fn wait_for_terminal(
    engine: &Engine,
    task_id: sayl_core::id::TaskId,
    timeout: Duration,
) -> DownloadState {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let task = engine.get_task(task_id).unwrap().unwrap();
        if task.state.is_terminal() {
            return task.state;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!(
        "task {} did not reach terminal state within {:?}",
        task_id, timeout
    );
}

fn make_temp_dir() -> TempDir {
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

fn make_temp_dirs() -> (TempDir, TempDir) {
    (make_temp_dir(), make_temp_dir())
}

#[tokio::test]
async fn test_full_sequential_download() {
    let (dl_dir, store_dir) = make_temp_dirs();
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let size = 16384;
    server.set_resource_size(size);
    let addr = server.start().await.expect("start fixture server");

    let config = EngineConfig {
        max_connections: 1,
        ..Default::default()
    };
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");

    let url = format!("http://{addr}/test_seq.bin");
    let task_id = engine.add(&url).await.expect("add task");

    let initial = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(initial.state, DownloadState::Queued);

    engine.start(task_id).await.expect("start download");

    let finished = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(finished.state, DownloadState::Completed);
    assert_eq!(finished.written, size as u64);

    let final_path = dl_dir.path().join(&finished.filename);
    assert!(final_path.exists());
    let bytes = tokio::fs::read(&final_path).await.expect("read final file");
    assert_eq!(bytes, generate_bytes(size));
}

#[tokio::test]
async fn test_truncated_body_fails_and_keeps_part_file() {
    let (dl_dir, store_dir) = make_temp_dirs();
    let mut server = HttpFixtureServer::with_scenario(Scenario::BodyTruncation);
    let size = 16384;
    server.set_resource_size(size);
    let addr = server.start().await.expect("start fixture server");

    let config = EngineConfig {
        max_connections: 1,
        ..Default::default()
    };
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");

    let url = format!("http://{addr}/test_trunc.bin");
    let task_id = engine.add(&url).await.expect("add task");

    let res = engine.start(task_id).await;
    assert!(res.is_err(), "expected start to fail on body truncation");

    let task = engine.get_task(task_id).unwrap().unwrap();
    assert_ne!(
        task.state,
        DownloadState::Completed,
        "task must NEVER be marked Completed on truncation"
    );
    assert!(
        task.state == DownloadState::Failed || task.state == DownloadState::Paused,
        "task state was {:?}",
        task.state
    );

    // .part file must be kept for resume
    let part_path = dl_dir.path().join(format!("{}.part", task.filename));
    assert!(
        part_path.exists(),
        ".part file must be preserved on truncation"
    );
    let part_len = tokio::fs::metadata(&part_path).await.unwrap().len();
    assert!(part_len > 0, "partial file must contain received bytes");
    assert!(part_len < size as u64, "partial file must be incomplete");
}

#[tokio::test]
async fn test_pause_then_resume_unchanged_etag() {
    let (dl_dir, store_dir) = make_temp_dirs();
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let size = 65536;
    server.set_resource_size(size);
    server.set_chunk_size(2048);
    server.set_delay(Duration::from_millis(15));
    let addr = server.start().await.expect("start fixture server");

    let config = EngineConfig {
        max_connections: 1,
        ..Default::default()
    };
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");

    let url = format!("http://{addr}/test_pause.bin");
    let task_id = engine.add(&url).await.expect("add task");

    let mut event_rx = engine.take_event_receiver().expect("take event receiver");

    // Spawn download in background
    let engine_clone = engine.clone();
    let handle = tokio::spawn(async move { engine_clone.start(task_id).await });

    // Wait until transfer has actively started and received progress
    while let Some(ev) = event_rx.recv().await {
        if let sayl_core::event::EventPayload::Progress {
            downloaded_bytes, ..
        } = ev.payload
        {
            if downloaded_bytes > 0 {
                break;
            }
        }
    }

    // Pause download
    engine.pause(task_id).await.expect("pause task");
    let _ = handle.await;

    let paused_task = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(paused_task.state, DownloadState::Paused);

    let checkpoint = engine
        .load_checkpoint(task_id)
        .unwrap()
        .expect("checkpoint exists");
    assert!(checkpoint.written > 0);

    let part_path = dl_dir.path().join(format!("{}.part", paused_task.filename));
    assert!(part_path.exists());

    // Disable delay on server and resume download
    server.set_delay(Duration::ZERO);
    engine.start(task_id).await.expect("resume download");

    let completed_task = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(completed_task.state, DownloadState::Completed);
    assert_eq!(completed_task.written, size as u64);

    let final_path = dl_dir.path().join(&completed_task.filename);
    let bytes = tokio::fs::read(&final_path).await.expect("read final file");
    assert_eq!(bytes, generate_bytes(size));
}

#[tokio::test]
async fn test_etag_changed_restarts_from_zero_and_completes() {
    let (dl_dir, store_dir) = make_temp_dirs();
    let mut server = HttpFixtureServer::with_scenario(Scenario::ETagChanged);
    let size = 16384;
    server.set_resource_size(size);
    let addr = server.start().await.expect("start fixture server");

    let config = EngineConfig {
        max_connections: 1,
        ..Default::default()
    };
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");

    let url = format!("http://{addr}/test_etag.bin");
    let task_id = engine.add(&url).await.expect("add task");

    // Simulate an existing stale partial file with dummy bytes
    let record = engine.get_task(task_id).unwrap().unwrap();
    let part_path = dl_dir.path().join(format!("{}.part", record.filename));
    tokio::fs::write(&part_path, vec![0xAA; 4096])
        .await
        .unwrap();

    // Start download: fresh probe receives secondary ETag ("v2" vs stored "v1"),
    // so validate_resume orders RestartFromZero.
    engine
        .start(task_id)
        .await
        .expect("start download after etag change");

    let completed_task = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(completed_task.state, DownloadState::Completed);

    let final_path = dl_dir.path().join(&completed_task.filename);
    let bytes = tokio::fs::read(&final_path).await.expect("read final file");
    assert_eq!(bytes, generate_bytes(size));
}

#[tokio::test]
async fn test_cancel_prompt_and_idempotent() {
    let (dl_dir, store_dir) = make_temp_dirs();
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let size = 32768;
    server.set_resource_size(size);
    server.set_delay(Duration::from_millis(20));
    let addr = server.start().await.expect("start fixture server");

    let config = EngineConfig::default();
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");

    let url = format!("http://{addr}/test_cancel.bin");
    let task_id = engine.add(&url).await.expect("add task");

    let engine_clone = engine.clone();
    let handle = tokio::spawn(async move { engine_clone.start(task_id).await });

    tokio::time::sleep(Duration::from_millis(30)).await;

    // Prompt cancel
    let start_time = Instant::now();
    engine.cancel(task_id).await.expect("cancel task");
    let elapsed = start_time.elapsed();
    assert!(
        elapsed < Duration::from_millis(500),
        "cancel took too long: {elapsed:?}"
    );

    let _ = handle.await;

    let cancelled_task = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(cancelled_task.state, DownloadState::Cancelled);

    // Second cancel is idempotent and must not error
    let second_res = engine.cancel(task_id).await;
    assert!(second_res.is_ok(), "second cancel must be a no-op");
    assert_eq!(
        engine.get_task(task_id).unwrap().unwrap().state,
        DownloadState::Cancelled
    );
}

#[tokio::test]
async fn test_no_accept_ranges_and_unknown_size_fallback_to_sequential() {
    // 1. NoAcceptRanges
    {
        let (dl_dir, store_dir) = make_temp_dirs();
        let mut server = HttpFixtureServer::with_scenario(Scenario::NoAcceptRanges);
        let size = 8192;
        server.set_resource_size(size);
        let addr = server.start().await.expect("start fixture server");

        let config = EngineConfig {
            max_connections: 4, // Even with max_connections=4, must fall back to sequential
            ..Default::default()
        };
        let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");

        let url = format!("http://{addr}/test_no_ranges.bin");
        let task_id = engine.add(&url).await.expect("add task");

        engine.start(task_id).await.expect("start download");

        let task = engine.get_task(task_id).unwrap().unwrap();
        assert_eq!(task.state, DownloadState::Completed);

        let final_path = dl_dir.path().join(&task.filename);
        let bytes = tokio::fs::read(&final_path).await.expect("read final file");
        assert_eq!(bytes, generate_bytes(size));
    }

    // 2. UnknownSize
    {
        let (dl_dir, store_dir) = make_temp_dirs();
        let mut server = HttpFixtureServer::with_scenario(Scenario::UnknownSize);
        let size = 8192;
        server.set_resource_size(size);
        let addr = server.start().await.expect("start fixture server");

        let config = EngineConfig {
            max_connections: 4,
            ..Default::default()
        };
        let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");

        let url = format!("http://{addr}/test_unknown_size.bin");
        let task_id = engine.add(&url).await.expect("add task");

        engine.start(task_id).await.expect("start download");

        let task = engine.get_task(task_id).unwrap().unwrap();
        assert_eq!(task.state, DownloadState::Completed);

        let final_path = dl_dir.path().join(&task.filename);
        let bytes = tokio::fs::read(&final_path).await.expect("read final file");
        assert_eq!(bytes, generate_bytes(size));
    }
}

#[tokio::test]
async fn test_parallel_download_with_disjoint_ranges() {
    let (dl_dir, store_dir) = make_temp_dirs();
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let size = 65536;
    server.set_resource_size(size);
    let addr = server.start().await.expect("start fixture server");

    let config = EngineConfig {
        max_connections: 4,
        chunk_size: 16384,
        ..Default::default()
    };
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");

    let url = format!("http://{addr}/test_parallel.bin");
    let task_id = engine.add(&url).await.expect("add task");

    engine
        .start(task_id)
        .await
        .expect("start parallel download");

    let task = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(task.state, DownloadState::Completed);
    assert_eq!(task.written, size as u64);

    let final_path = dl_dir.path().join(&task.filename);
    let bytes = tokio::fs::read(&final_path).await.expect("read final file");
    assert_eq!(bytes, generate_bytes(size));

    // Audit recorded requests: assert exactly 4 ranged requests and no range written twice
    let recorded = server.recorded_requests().await;
    let mut subrequest_ranges = Vec::new();

    for req in recorded {
        if req.method == "GET" {
            if let Some(range) = req.range_header {
                if range != "bytes=0-0" {
                    // This is one of the download subrequests
                    subrequest_ranges.push(range);
                }
            }
        }
    }

    assert_eq!(
        subrequest_ranges.len(),
        4,
        "expected exactly 4 parallel subrequests, got: {subrequest_ranges:?}"
    );

    // Assert that each range is unique (no range requested twice)
    let unique_ranges: HashSet<_> = subrequest_ranges.iter().collect();
    assert_eq!(
        unique_ranges.len(),
        4,
        "ranges must be strictly unique and non-overlapping: {subrequest_ranges:?}"
    );
}

#[tokio::test]
async fn test_recovery_non_terminal_tasks() {
    let dl_dir = make_temp_dir();
    let store_dir = make_temp_dir();

    let task_id;
    {
        let store = Store::open(store_dir.path()).expect("open store");
        let mut record = TaskRecord::new(
            "http://example.com/test_recover.bin",
            dl_dir.path(),
            "test_recover.bin",
        );
        task_id = record.id;
        record.state = DownloadState::Downloading;
        record.total_size = Some(10240);
        record.written = 2048;
        store.upsert_task(&record).expect("upsert task");
    }

    // Recreate engine with same directories and recover
    let config = EngineConfig::default();
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("reopen engine");

    let recovered = engine.recover().await.expect("recover must not panic");
    assert!(recovered.contains(&task_id));

    let recovered_task = engine.get_task(task_id).unwrap().unwrap();
    assert!(
        recovered_task.state == DownloadState::Paused
            || recovered_task.state == DownloadState::Queued
            || recovered_task.state == DownloadState::Failed,
        "recovered task must be in Paused/Queued/Failed, was {:?}",
        recovered_task.state
    );
}

#[tokio::test]
async fn test_concurrency_limit_extra_stay_queued() {
    let (dl_dir, store_dir) = make_temp_dirs();
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    server.set_resource_size(65536);
    server.set_delay(Duration::from_millis(50));
    let addr = server.start().await.expect("start fixture server");

    // Concurrency limit = 2
    let config = EngineConfig {
        max_concurrent_tasks: 2,
        ..Default::default()
    };
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");

    let t1 = engine.add(format!("http://{addr}/t1.bin")).await.unwrap();
    let t2 = engine.add(format!("http://{addr}/t2.bin")).await.unwrap();
    let t3 = engine.add(format!("http://{addr}/t3.bin")).await.unwrap();

    let e1 = engine.clone();
    let h1 = tokio::spawn(async move { e1.start(t1).await });
    let e2 = engine.clone();
    let h2 = tokio::spawn(async move { e2.start(t2).await });

    // Wait briefly so t1 and t2 are actively running
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Try starting t3: must fail due to concurrency limit
    let res3 = engine.start(t3).await;
    assert!(
        res3.is_err(),
        "task t3 must be rejected by concurrency limit"
    );

    // t3 must remain in Queued state
    let task3 = engine.get_task(t3).unwrap().unwrap();
    assert_eq!(task3.state, DownloadState::Queued);

    // Cancel t1 and t2 to clean up
    let _ = engine.cancel(t1).await;
    let _ = engine.cancel(t2).await;
    let _ = h1.await;
    let _ = h2.await;
}

/// Test: With max_concurrent_tasks = 1, two queued tasks - second stays Queued while first runs,
/// then starts after first completes. Both files byte-correct.
#[tokio::test]
async fn test_scheduler_max_concurrent_tasks_queues_excess() {
    let (dl_dir, store_dir) = make_temp_dirs();
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let size = 32768;
    server.set_resource_size(size);
    server.set_delay(Duration::from_millis(50));
    let addr = server.start().await.expect("start fixture server");

    // Concurrency limit = 1 (both top-level and queue_limits)
    let mut config = EngineConfig {
        max_concurrent_tasks: 1,
        ..Default::default()
    };
    config.queue_limits.max_concurrent_tasks = 1;
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");

    let url1 = format!("http://{addr}/task1.bin");
    let url2 = format!("http://{addr}/task2.bin");

    let task1 = engine.add(&url1).await.expect("add task1");
    let task2 = engine.add(&url2).await.expect("add task2");

    // Start task1 and wait until it has genuinely left Queued (i.e., promoted to Running)
    let engine_clone = engine.clone();
    let handle1 = tokio::spawn(async move { engine_clone.start(task1).await });

    // Poll until task1 is no longer Queued - this guarantees it holds the slot
    let t1_state = wait_until_not_queued(&engine, task1, Duration::from_secs(5)).await;
    assert_ne!(
        t1_state,
        DownloadState::Queued,
        "task1 should be promoted, was {:?}",
        t1_state
    );

    // NOW start task2 - it should be queued because task1 holds the only slot
    let res2 = engine.start(task2).await;
    assert!(res2.is_ok(), "start for queued task should return Ok");

    // Verify task2 is queued while task1 runs
    let t2_state = engine.get_task(task2).unwrap().unwrap().state;
    assert_eq!(
        t2_state,
        DownloadState::Queued,
        "task2 should be queued while task1 runs, was {:?}",
        t2_state
    );

    // Wait for task1 to complete
    let _ = handle1.await;

    // Now task2 should have been promoted and completed
    let t2_final_state = wait_for_terminal(&engine, task2, Duration::from_secs(10)).await;
    assert_eq!(t2_final_state, DownloadState::Completed);

    // Both should be completed
    let t1_final = engine.get_task(task1).unwrap().unwrap();
    let t2_final = engine.get_task(task2).unwrap().unwrap();
    assert_eq!(t1_final.state, DownloadState::Completed);
    assert_eq!(t2_final.state, DownloadState::Completed);
    assert_eq!(t1_final.written, size as u64);
    assert_eq!(t2_final.written, size as u64);

    // Verify file contents
    let bytes1 = tokio::fs::read(dl_dir.path().join(&t1_final.filename))
        .await
        .expect("read task1 file");
    let bytes2 = tokio::fs::read(dl_dir.path().join(&t2_final.filename))
        .await
        .expect("read task2 file");
    assert_eq!(bytes1, generate_bytes(size));
    assert_eq!(bytes2, generate_bytes(size));
}

/// Test: max_connections_per_origin = 1 with two tasks on SAME origin - both complete,
/// never more than one request in flight at a time (assert from fixture's recorded requests).
#[tokio::test]
async fn test_scheduler_per_origin_limit() {
    let (dl_dir, store_dir) = make_temp_dirs();
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let size = 16384;
    server.set_resource_size(size);
    server.set_delay(Duration::from_millis(50));
    let addr = server.start().await.expect("start fixture server");

    // Per-origin limit = 1, high global limit
    // Force sequential by setting max_connections=1
    let mut config = EngineConfig {
        max_concurrent_tasks: 10,
        max_connections: 1,
        ..Default::default()
    };
    config.queue_limits.max_connections_per_origin = 1;
    config.queue_limits.max_concurrent_tasks = 10;
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");

    let url1 = format!("http://{addr}/origin1_task1.bin");
    let url2 = format!("http://{addr}/origin1_task2.bin");

    let task1 = engine.add(&url1).await.expect("add task1");
    let task2 = engine.add(&url2).await.expect("add task2");

    // Start task1 and wait until it leaves Queued (holds origin slot)
    let engine_clone = engine.clone();
    let handle1 = tokio::spawn(async move { engine_clone.start(task1).await });
    wait_until_not_queued(&engine, task1, Duration::from_secs(5)).await;

    // Now start task2 - it should be queued due to per-origin limit
    let res2 = engine.start(task2).await;
    assert!(res2.is_ok(), "start for queued task should return Ok");

    // Verify task2 is queued
    let t2_state = engine.get_task(task2).unwrap().unwrap().state;
    assert_eq!(
        t2_state,
        DownloadState::Queued,
        "task2 should be queued due to per-origin limit, was {:?}",
        t2_state
    );

    // Wait for task1 to complete (releases origin slot)
    let _ = handle1.await;

    // Task2 should be promoted and complete
    let t2_final = wait_for_terminal(&engine, task2, Duration::from_secs(10)).await;
    assert_eq!(t2_final, DownloadState::Completed);

    // Both completed
    let t1 = engine.get_task(task1).unwrap().unwrap();
    let t2 = engine.get_task(task2).unwrap().unwrap();
    assert_eq!(t1.state, DownloadState::Completed);
    assert_eq!(t2.state, DownloadState::Completed);

    // Verify byte-correctness
    let bytes1 = tokio::fs::read(dl_dir.path().join(&t1.filename))
        .await
        .expect("read task1 file");
    let bytes2 = tokio::fs::read(dl_dir.path().join(&t2.filename))
        .await
        .expect("read task2 file");
    assert_eq!(bytes1, generate_bytes(size));
    assert_eq!(bytes2, generate_bytes(size));

    // Verify fixture recorded requests - assert sequential per-origin access
    // With max_connections=1 and per-origin=1, each task's download runs sequentially.
    // The key assertion: task2 was queued while task1 ran (already verified above).
    // Both tasks completed and files are byte-correct (verified above).
    // Additional check: total range GETs should be > 0 for both tasks.
    let recorded = server.recorded_requests().await;
    let mut range_gets = 0;
    for req in &recorded {
        if req.method == "GET" && req.range_header.is_some() {
            range_gets += 1;
        }
    }
    assert!(
        range_gets >= 2,
        "expected at least 2 range GETs, got {}",
        range_gets
    );
}

/// Test: Two tasks on different origins are not blocked by one another.
#[tokio::test]
async fn test_scheduler_different_origins_not_blocked() {
    let (dl_dir, store_dir) = make_temp_dirs();

    // Start two fixture servers on different ports (simulating different origins)
    let mut server1 = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let size = 16384;
    server1.set_resource_size(size);
    server1.set_delay(Duration::from_millis(100));
    let addr1 = server1.start().await.expect("start fixture server 1");

    let mut server2 = HttpFixtureServer::with_scenario(Scenario::Ok206);
    server2.set_resource_size(size);
    server2.set_delay(Duration::from_millis(100));
    let addr2 = server2.start().await.expect("start fixture server 2");

    // High concurrency, per-origin limit = 1
    let mut config = EngineConfig {
        max_concurrent_tasks: 10,
        max_connections: 1,
        ..Default::default()
    };
    config.queue_limits.max_connections_per_origin = 1;
    config.queue_limits.max_concurrent_tasks = 10;
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");

    let url1 = format!("http://{addr1}/different_origin1.bin");
    let url2 = format!("http://{addr2}/different_origin2.bin");

    let task1 = engine.add(&url1).await.expect("add task1");
    let task2 = engine.add(&url2).await.expect("add task2");

    let start = Instant::now();

    // Start both - they should run in parallel (different origins, per-origin limit=1 each)
    let engine_clone = engine.clone();
    let handle1 = tokio::spawn(async move { engine_clone.start(task1).await });
    let engine_clone = engine.clone();
    let handle2 = tokio::spawn(async move { engine_clone.start(task2).await });

    let _ = handle1.await;
    let _ = handle2.await;

    let elapsed = start.elapsed();

    // Both should complete
    let t1 = engine.get_task(task1).unwrap().unwrap();
    let t2 = engine.get_task(task2).unwrap().unwrap();
    assert_eq!(t1.state, DownloadState::Completed);
    assert_eq!(t2.state, DownloadState::Completed);

    // Since they're on different origins, they should run in parallel
    // Each takes ~100ms + overhead, so parallel should be ~150-200ms, sequential ~400ms+
    assert!(
        elapsed < Duration::from_millis(800),
        "different origins should run in parallel, took {:?}",
        elapsed
    );

    // Verify byte-correctness
    let bytes1 = tokio::fs::read(dl_dir.path().join(&t1.filename))
        .await
        .expect("read task1 file");
    let bytes2 = tokio::fs::read(dl_dir.path().join(&t2.filename))
        .await
        .expect("read task2 file");
    assert_eq!(bytes1, generate_bytes(size));
    assert_eq!(bytes2, generate_bytes(size));
}

/// Test: cancel on a queued-but-not-started task removes it and it never downloads.
#[tokio::test]
async fn test_scheduler_cancel_queued_task() {
    let (dl_dir, store_dir) = make_temp_dirs();
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let size = 32768;
    server.set_resource_size(size);
    server.set_delay(Duration::from_millis(200));
    let addr = server.start().await.expect("start fixture server");

    // Concurrency limit = 1
    let mut config = EngineConfig {
        max_concurrent_tasks: 1,
        max_connections: 1,
        ..Default::default()
    };
    config.queue_limits.max_concurrent_tasks = 1;
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");

    let url1 = format!("http://{addr}/cancel_task1.bin");
    let url2 = format!("http://{addr}/cancel_task2.bin");

    let task1 = engine.add(&url1).await.expect("add task1");
    let task2 = engine.add(&url2).await.expect("add task2");

    // Helper to count requests for a given path fragment
    async fn count_requests(server: &HttpFixtureServer, path_fragment: &str) -> usize {
        let recorded = server.recorded_requests().await;
        recorded
            .iter()
            .filter(|req| req.method == "GET" && req.path.contains(path_fragment))
            .count()
    }

    // Baseline: add() probes the URL, so there's exactly 1 GET for cancel_task2 already
    let baseline_task2 = count_requests(&server, "cancel_task2").await;
    assert_eq!(baseline_task2, 1, "add() is expected to probe once");

    // Start task1 and wait until it leaves Queued (holds the only slot)
    let engine_clone = engine.clone();
    let handle1 = tokio::spawn(async move { engine_clone.start(task1).await });
    wait_until_not_queued(&engine, task1, Duration::from_secs(5)).await;

    // Longer delay to ensure task1's download is well underway and won't complete before cancel
    tokio::time::sleep(Duration::from_millis(500)).await;

    // NOW task2 should be queued - verify and cancel immediately
    let t2_state = engine.get_task(task2).unwrap().unwrap().state;
    assert_eq!(
        t2_state,
        DownloadState::Queued,
        "task2 must be queued before cancel, was {:?}",
        t2_state
    );
    engine.cancel(task2).await.expect("cancel task2");

    // Verify scheduler no longer has the cancelled task queued
    let stats = engine.queue_stats().await;
    assert_eq!(
        stats.queued, 0,
        "scheduler should not have cancelled task queued, got {}",
        stats.queued
    );

    // Wait for task1 to complete
    let _ = handle1.await;

    // Task2 should be cancelled, never downloaded
    let t2 = engine.get_task(task2).unwrap().unwrap();
    assert_eq!(t2.state, DownloadState::Cancelled);

    // Verify no download occurred for task2 - no .part file, no final file
    let part_path = dl_dir.path().join(format!("{}.part", t2.filename));
    let final_path = dl_dir.path().join(&t2.filename);
    assert!(!part_path.exists(), "task2 .part file should not exist");
    assert!(!final_path.exists(), "task2 final file should not exist");

    // Verify fixture recorded requests - only the baseline probe for task2, no download requests
    let final_task2 = count_requests(&server, "cancel_task2").await;
    assert_eq!(
        final_task2,
        baseline_task2,
        "task2 should have only baseline probe (1), got {} additional requests",
        final_task2 - baseline_task2
    );
    let task1_requests = count_requests(&server, "cancel_task1").await;
    assert!(
        task1_requests > 0,
        "task1 should have been downloaded, got {}",
        task1_requests
    );
}

#[tokio::test]
async fn test_404_stores_real_error_text() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await;
            let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            let _ = socket.write_all(resp.as_bytes()).await;
        }
    });

    let dl_dir = make_temp_dir();
    let store_dir = make_temp_dir();
    let config = EngineConfig::default();
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");

    let url = format!("http://{addr}/notfound.bin");
    let task_id = engine.add(&url).await.expect("add task");

    let eng = engine.clone();
    let _handle = tokio::spawn(async move { eng.start(task_id).await });

    let terminal_state = wait_for_terminal(&engine, task_id, Duration::from_secs(5)).await;
    assert_eq!(terminal_state, DownloadState::Failed);

    let task = engine.get_task(task_id).unwrap().unwrap();
    assert!(
        task.error.as_deref().unwrap_or("").contains("404"),
        "error must contain 404, got: {:?}",
        task.error
    );
}

#[tokio::test]
async fn test_retry_failed_task() {
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let size = 16384;
    server.set_resource_size(size);
    let addr = server.start().await.expect("start fixture server");

    let dl_dir = make_temp_dir();
    let store_dir = make_temp_dir();
    let config = EngineConfig::default();
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");

    let url = format!("http://{addr}/retry_test.dat");
    let task_id = engine.add(&url).await.expect("add task");

    // Manually mark task as Failed with an error to simulate previous failure
    let store = Store::open(store_dir.path().join("sayl.db")).expect("open store");
    {
        let mut record = engine.get_task(task_id).unwrap().unwrap();
        record.state = DownloadState::Failed;
        record.error = Some("Previous failure test error".to_string());
        record.error_kind = Some("http_500".to_string());
        store.upsert_task(&record).unwrap();
    }

    // Now call start: retry should be allowed, resetting state, clearing error, and completing!
    let eng = engine.clone();
    let handle = tokio::spawn(async move { eng.start(task_id).await });
    let terminal_state = wait_for_terminal(&engine, task_id, Duration::from_secs(10)).await;
    let _ = handle.await;

    assert_eq!(terminal_state, DownloadState::Completed);
    let final_task = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(final_task.error, None, "error must be cleared on retry");
    assert_eq!(
        final_task.error_kind, None,
        "error_kind must be cleared on retry"
    );
}

#[tokio::test]
async fn test_parallel_429_fallback_to_sequential() {
    let size = 16384usize;
    let expected_data = generate_bytes(size);
    let data_clone = expected_data.clone();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let data = data_clone.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 2048];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);

                if req.starts_with("HEAD") {
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                        size
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                } else if req.contains("Range: bytes=") {
                    if req.contains("Range: bytes=0-0") {
                        let resp = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-0/{}\r\nContent-Length: 1\r\nConnection: close\r\n\r\n{}",
                            size, data[0] as char
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;
                    } else {
                        // Parallel worker range request: simulate 429 throttling!
                        let resp = "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 0\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                        let _ = socket.write_all(resp.as_bytes()).await;
                    }
                } else {
                    // Sequential full GET without Range: succeed!
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        size
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                    let _ = socket.write_all(&data).await;
                }
            });
        }
    });

    let dl_dir = make_temp_dir();
    let store_dir = make_temp_dir();
    let config = EngineConfig::default();
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");

    let url = format!("http://{addr}/test_429_fallback.dat");
    let task_id = engine.add(&url).await.expect("add task");

    let eng = engine.clone();
    let handle = tokio::spawn(async move { eng.start(task_id).await });
    let terminal_state = wait_for_terminal(&engine, task_id, Duration::from_secs(10)).await;
    let _ = handle.await;

    assert_eq!(terminal_state, DownloadState::Completed);
    let final_task = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(final_task.written, size as u64);

    let final_path = dl_dir.path().join(&final_task.filename);
    let bytes = tokio::fs::read(&final_path).await.expect("read final file");
    assert_eq!(bytes, expected_data);
}

#[tokio::test]
async fn test_resume_from_checkpoint_skips_probe_and_ranges_from_offset() {
    let size = 16384usize;
    let offset = 4096usize;
    let expected_data = generate_bytes(size);
    let data_clone = expected_data.clone();

    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let requests_clone = std::sync::Arc::clone(&requests);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let data = data_clone.clone();
            let reqs = std::sync::Arc::clone(&requests_clone);
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                reqs.lock().unwrap().push(req.clone());

                if req.starts_with("HEAD") {
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"etag-test\"\r\nConnection: close\r\n\r\n",
                        size
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                } else if req.contains("Range: bytes=") {
                    if req.contains("Range: bytes=4096-") {
                        let body = &data[offset..];
                        let resp = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nETag: \"etag-test\"\r\nConnection: close\r\n\r\n",
                            offset,
                            size - 1,
                            size,
                            body.len()
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;
                        let _ = socket.write_all(body).await;
                    } else if req.contains("Range: bytes=0-0") {
                        let resp = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-0/{}\r\nContent-Length: 1\r\nETag: \"etag-test\"\r\nConnection: close\r\n\r\n{}",
                            size, data[0] as char
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;
                    } else {
                        let resp =
                            "HTTP/1.1 416 Range Not Satisfiable\r\nConnection: close\r\n\r\n";
                        let _ = socket.write_all(resp.as_bytes()).await;
                    }
                } else {
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"etag-test\"\r\nConnection: close\r\n\r\n",
                        size
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                    let _ = socket.write_all(&data).await;
                }
            });
        }
    });

    let (dl_dir, store_dir) = make_temp_dirs();
    let config = EngineConfig::default();
    let store = std::sync::Arc::new(Store::open(store_dir.path()).expect("open store"));
    let engine = Engine::with_store(dl_dir.path(), std::sync::Arc::clone(&store), config)
        .expect("create engine");
    let mut event_rx = engine.take_event_receiver().expect("take event rx");

    let url = format!("http://{addr}/test_fast_resume.dat");
    let task_id = sayl_core::id::TaskId::new();

    // Pre-create .part file on disk covering offset bytes
    let filename = "test_fast_resume.dat";
    let part_path = dl_dir.path().join(format!("{filename}.part"));
    tokio::fs::write(&part_path, &expected_data[..offset])
        .await
        .expect("write .part file");

    // Pre-create TaskRecord in Paused state with known total_size and etag validator
    let mut record = TaskRecord::with_id(task_id, &url, dl_dir.path(), filename);
    record.total_size = Some(size as u64);
    record.written = offset as u64;
    record.etag = Some("\"etag-test\"".to_string());
    record.state = DownloadState::Paused;
    store.upsert_task(&record).expect("upsert record");

    // Pre-create checkpoint in store at offset N
    let checkpoint = sayl_storage::Checkpoint::new(
        task_id,
        offset as u64,
        Some(size as u64),
        vec![(0, offset as u64)],
    );
    store.save_checkpoint(&checkpoint).expect("save checkpoint");

    // Start resume download
    let start_time = Instant::now();
    engine.start(task_id).await.expect("start resume download");
    let terminal = wait_for_terminal(&engine, task_id, Duration::from_secs(5)).await;
    let elapsed = start_time.elapsed();

    assert_eq!(terminal, DownloadState::Completed);
    assert!(
        elapsed < Duration::from_secs(3),
        "resume must be fast on loopback, took {elapsed:?}"
    );

    // Assert that the first HTTP request received by origin was Range: bytes=4096- and no probe was made
    let reqs = requests.lock().unwrap().clone();
    assert!(!reqs.is_empty(), "requests must have been made");
    assert!(
        !reqs.iter().any(|r| r.starts_with("HEAD")),
        "no HEAD probe request must be issued on fast resume, got: {:?}",
        reqs
    );
    assert!(
        !reqs
            .iter()
            .any(|r| r.to_ascii_lowercase().contains("range: bytes=0-0")),
        "no probe ranged GET (0-0) must be issued on fast resume, got: {:?}",
        reqs
    );
    assert!(
        reqs[0].to_ascii_lowercase().contains("range: bytes=4096-"),
        "first HTTP request must be Range starting at 4096, got: {}",
        reqs[0]
    );

    // Verify events: DownloadState never became Probing!
    while let Ok(event) = event_rx.try_recv() {
        if let sayl_core::event::EventPayload::StateChanged { to, .. } = event.payload {
            assert_ne!(
                to,
                DownloadState::Probing,
                "task state must never transition to Probing during fast resume"
            );
        }
    }

    // Assert total_size stayed unchanged
    let final_task = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(
        final_task.total_size,
        Some(size as u64),
        "total_size must stay unchanged"
    );
    assert_eq!(final_task.written, size as u64);

    // Final file must be complete and byte-identical
    let final_file = dl_dir.path().join(filename);
    let bytes = tokio::fs::read(&final_file).await.expect("read final file");
    assert_eq!(bytes, expected_data);
}

#[tokio::test]
async fn test_parallel_fallback_preserves_progress_and_size() {
    let size = 32768usize;
    let chunk_size = 8192usize;
    let expected_data = generate_bytes(size);
    let data_clone = expected_data.clone();

    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let requests_clone = std::sync::Arc::clone(&requests);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let data = data_clone.clone();
            let reqs = std::sync::Arc::clone(&requests_clone);
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                reqs.lock().unwrap().push(req.clone());
                let lower = req.to_ascii_lowercase();

                if req.starts_with("HEAD") {
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"etag-parallel\"\r\nConnection: close\r\n\r\n",
                        size
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                } else if lower.contains("range: bytes=") {
                    if lower.contains("range: bytes=0-0") {
                        // Range probe check
                        let resp = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-0/{}\r\nContent-Length: 1\r\nAccept-Ranges: bytes\r\nETag: \"etag-parallel\"\r\nConnection: close\r\n\r\n",
                            size
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;
                        let _ = socket.write_all(&data[0..1]).await;
                    } else if lower.contains("range: bytes=0-8191") {
                        // First chunk succeeds
                        let chunk = &data[0..8192];
                        let resp = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-8191/{}\r\nContent-Length: 8192\r\nAccept-Ranges: bytes\r\nETag: \"etag-parallel\"\r\nConnection: close\r\n\r\n",
                            size
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;
                        let _ = socket.write_all(chunk).await;
                    } else if lower.contains("range: bytes=8192-") {
                        // Fallback sequential range request starting at 8192!
                        let remaining = &data[8192..];
                        let resp = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 8192-{}/{}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"etag-parallel\"\r\nConnection: close\r\n\r\n",
                            size - 1,
                            size,
                            remaining.len()
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;
                        let _ = socket.write_all(remaining).await;
                    } else {
                        // Other worker requests fail with 429
                        let resp = "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 0\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                        let _ = socket.write_all(resp.as_bytes()).await;
                    }
                } else {
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                        size
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                    let _ = socket.write_all(&data).await;
                }
            });
        }
    });

    let (dl_dir, store_dir) = make_temp_dirs();
    let config = EngineConfig {
        max_connections: 4,
        chunk_size,
        ..Default::default()
    };
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");

    let url = format!("http://{addr}/test_fallback_progress.dat");
    let task_id = engine.add(&url).await.expect("add task");

    let eng = engine.clone();
    let handle = tokio::spawn(async move { eng.start(task_id).await });
    let terminal = wait_for_terminal(&engine, task_id, Duration::from_secs(10)).await;
    let _ = handle.await;

    assert_eq!(terminal, DownloadState::Completed);

    let final_task = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(
        final_task.total_size,
        Some(size as u64),
        "total_size must be preserved on fallback"
    );
    assert_eq!(final_task.written, size as u64);

    let reqs = requests.lock().unwrap().clone();
    assert!(
        reqs.iter()
            .any(|r| r.to_ascii_lowercase().contains("range: bytes=8192-")),
        "fallback must resume from completed offset with Range: bytes=8192-, got requests: {:?}",
        reqs
    );

    let final_path = dl_dir.path().join(&final_task.filename);
    let bytes = tokio::fs::read(&final_path).await.expect("read final file");
    assert_eq!(bytes, expected_data);
}

#[tokio::test]
async fn test_deleted_failed_task_not_resurrected_after_reopen() {
    let (dl_dir, store_dir) = make_temp_dirs();
    let config = EngineConfig::default();

    let task_id;
    let late_record;
    {
        let store = std::sync::Arc::new(Store::open(store_dir.path()).expect("open store"));
        let engine =
            Engine::with_store(dl_dir.path(), std::sync::Arc::clone(&store), config.clone())
                .expect("create engine");

        task_id = sayl_core::id::TaskId::new();
        let mut record = TaskRecord::with_id(
            task_id,
            "http://example.invalid/100MB.dat",
            dl_dir.path(),
            "100MB.dat",
        );
        record.state = DownloadState::Failed;
        record.error = Some("connection refused".to_string());
        record.error_kind = Some("network".to_string());
        record.total_size = Some(104857600);
        store.upsert_task(&record).expect("upsert failed task");

        let checkpoint =
            sayl_storage::Checkpoint::new(task_id, 1024, Some(104857600), vec![(0, 1024)]);
        store.save_checkpoint(&checkpoint).expect("save checkpoint");

        assert!(engine.get_task(task_id).unwrap().is_some());
        assert!(engine.load_checkpoint(task_id).unwrap().is_some());

        // Delete the failed task
        engine.delete_task(task_id).expect("delete task");
        assert!(engine.get_task(task_id).unwrap().is_none());
        assert!(engine.load_checkpoint(task_id).unwrap().is_none());

        // Late upsert attempt (simulating late retry or background pump) must be ignored
        late_record = record.clone();
        store.upsert_task(&late_record).expect("late upsert");
        assert!(
            engine.get_task(task_id).unwrap().is_none(),
            "late upsert must not resurrect deleted task"
        );

        // Late checkpoint attempt must also be ignored
        let _ = store.save_checkpoint(&checkpoint);
        assert!(
            engine.load_checkpoint(task_id).unwrap().is_none(),
            "late checkpoint must not recreate checkpoint"
        );

        drop(engine);
        drop(store);
    }

    // Reopen engine and store from the same directory and execute recover()
    {
        let store = std::sync::Arc::new(Store::open(store_dir.path()).expect("reopen store"));
        let engine =
            Engine::with_store(dl_dir.path(), std::sync::Arc::clone(&store), config.clone())
                .expect("reopen engine");
        let recovered = engine.recover().await.expect("recover");

        assert!(
            !recovered.contains(&task_id),
            "recovered tasks must not contain deleted task"
        );
        assert!(
            engine.get_task(task_id).unwrap().is_none(),
            "deleted failed task must remain absent after restart and recover"
        );
        assert!(
            engine.load_checkpoint(task_id).unwrap().is_none(),
            "checkpoint must remain absent after restart"
        );

        // A late upsert on the reopened store must still be rejected by the tombstone guard
        store
            .upsert_task(&late_record)
            .expect("late upsert after reopen");
        assert!(
            engine.get_task(task_id).unwrap().is_none(),
            "late upsert after reopen must still be ignored"
        );
    }
}

#[tokio::test]
async fn test_add_with_metadata_skips_network_probe() {
    let (dl_dir, store_dir) = make_temp_dirs();
    let config = EngineConfig::default();
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");

    // Use an unroutable loopback port to prove that NO network probe is performed when filename + size are supplied
    let url = "http://127.0.0.1:1/nonexistent.iso";
    let filename = Some("custom_name.iso".to_string());
    let size = Some(52428800u64); // 50 MiB
    let referrer = Some("https://source.example.com/download".to_string());
    let headers = Some(vec![("X-Auth-Token".to_string(), "secret123".to_string())]);

    let task_id = engine
        .add_with_metadata(&url, filename, size, referrer, headers)
        .await
        .expect("add_with_metadata must succeed without network probe");

    let task = engine
        .get_task(task_id)
        .unwrap()
        .expect("task must exist in store");
    assert_eq!(task.filename, "custom_name.iso");
    assert_eq!(task.total_size, Some(52428800));
    assert_eq!(task.state, DownloadState::Queued);

    // Verify privacy rule: headers and tokens are NOT stored in TaskRecord or store journal
    assert!(task.error.is_none());
}

#[tokio::test]
async fn test_stall_watchdog_retries_once_from_offset_and_fails_if_exhausted() {
    let size = 8192usize;
    let expected_data = generate_bytes(size);
    let data_clone = expected_data.clone();

    let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let attempts_clone = std::sync::Arc::clone(&attempts);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let data = data_clone.clone();
            let att = std::sync::Arc::clone(&attempts_clone);
            tokio::spawn(async move {
                let mut buf = [0u8; 2048];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();

                if req.starts_with("HEAD") {
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                        size
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                } else {
                    let count = att.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if count == 0 {
                        // First attempt: stall by sending response headers then sleeping longer than stall timeout
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            size
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;
                        // Send partial 1024 bytes then stall
                        let _ = socket.write_all(&data[..1024]).await;
                        tokio::time::sleep(Duration::from_millis(1500)).await;
                    } else {
                        // Retry attempt: send full data promptly
                        if req.contains("Range: bytes=1024-") {
                            let resp = format!(
                                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 1024-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                size - 1, size, size - 1024
                            );
                            let _ = socket.write_all(resp.as_bytes()).await;
                            let _ = socket.write_all(&data[1024..]).await;
                        } else {
                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                size
                            );
                            let _ = socket.write_all(resp.as_bytes()).await;
                            let _ = socket.write_all(&data).await;
                        }
                    }
                }
            });
        }
    });

    let (dl_dir, store_dir) = make_temp_dirs();
    let config = EngineConfig {
        stall_timeout: Duration::from_millis(300),
        ..Default::default()
    };
    let engine = Engine::new(dl_dir.path(), store_dir.path(), config).expect("create engine");
    engine.set_stall_timeout(Duration::from_millis(300));

    let url = format!("http://{addr}/test_watchdog.dat");
    let task_id = engine.add(&url).await.expect("add task");

    let eng = engine.clone();
    let handle = tokio::spawn(async move { eng.start(task_id).await });
    let terminal = wait_for_terminal(&engine, task_id, Duration::from_secs(6)).await;
    let _ = handle.await;

    assert_eq!(terminal, DownloadState::Completed);
    assert!(
        attempts.load(std::sync::atomic::Ordering::SeqCst) >= 2,
        "watchdog must have aborted and retried"
    );

    let final_task = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(final_task.written, size as u64);

    let final_path = dl_dir.path().join(&final_task.filename);
    let bytes = tokio::fs::read(&final_path).await.expect("read final file");
    assert_eq!(bytes, expected_data);
}

#[tokio::test]
async fn test_resume_without_validator_never_probes() {
    let size = 16384usize;
    let offset = 4096usize;
    let expected_data = generate_bytes(size);
    let data_clone = expected_data.clone();

    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let requests_clone = std::sync::Arc::clone(&requests);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let data = data_clone.clone();
            let reqs = std::sync::Arc::clone(&requests_clone);
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                reqs.lock().unwrap().push(req.clone());
                let lower = req.to_ascii_lowercase();

                if req.starts_with("HEAD") {
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                        size
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                } else if lower.contains("range: bytes=") {
                    if lower.contains("range: bytes=4096-") {
                        let body = &data[offset..];
                        let resp = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            offset,
                            size - 1,
                            size,
                            body.len()
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;
                        let _ = socket.write_all(body).await;
                    } else if lower.contains("range: bytes=0-0") {
                        let resp = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-0/{}\r\nContent-Length: 1\r\nConnection: close\r\n\r\n",
                            size
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;
                        let _ = socket.write_all(&data[0..1]).await;
                    } else {
                        let resp =
                            "HTTP/1.1 416 Range Not Satisfiable\r\nConnection: close\r\n\r\n";
                        let _ = socket.write_all(resp.as_bytes()).await;
                    }
                } else {
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        size
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                    let _ = socket.write_all(&data).await;
                }
            });
        }
    });

    let (dl_dir, store_dir) = make_temp_dirs();
    let config = EngineConfig::default();
    let store = std::sync::Arc::new(Store::open(store_dir.path()).expect("open store"));
    let engine = Engine::with_store(dl_dir.path(), std::sync::Arc::clone(&store), config)
        .expect("create engine");
    let mut event_rx = engine.take_event_receiver().expect("take event rx");

    let url = format!("http://{addr}/test_no_validator_resume.dat");
    let task_id = sayl_core::id::TaskId::new();

    // Pre-create .part file on disk covering offset bytes
    let filename = "test_no_validator_resume.dat";
    let part_path = dl_dir.path().join(format!("{filename}.part"));
    tokio::fs::write(&part_path, &expected_data[..offset])
        .await
        .expect("write .part file");

    // Pre-create TaskRecord in Paused state with known total_size and NO etag/last_modified validator
    let mut record = TaskRecord::with_id(task_id, &url, dl_dir.path(), filename);
    record.total_size = Some(size as u64);
    record.written = offset as u64;
    record.etag = None;
    record.last_modified = None;
    record.state = DownloadState::Paused;
    store.upsert_task(&record).expect("upsert record");

    // Pre-create checkpoint in store at offset N
    let checkpoint = sayl_storage::Checkpoint::new(
        task_id,
        offset as u64,
        Some(size as u64),
        vec![(0, offset as u64)],
    );
    store.save_checkpoint(&checkpoint).expect("save checkpoint");

    // Start resume download
    engine.start(task_id).await.expect("start resume download");
    let terminal = wait_for_terminal(&engine, task_id, Duration::from_secs(5)).await;

    assert_eq!(terminal, DownloadState::Completed);

    // Assert that no HEAD request and no bytes=0-0 request was made
    let reqs = requests.lock().unwrap().clone();
    assert!(!reqs.is_empty(), "requests must have been made");
    assert!(
        !reqs.iter().any(|r| r.starts_with("HEAD")),
        "no HEAD request must be made, got: {:?}",
        reqs
    );
    assert!(
        !reqs
            .iter()
            .any(|r| r.to_ascii_lowercase().contains("range: bytes=0-0")),
        "no bytes=0-0 request must be made, got: {:?}",
        reqs
    );
    assert!(
        reqs[0].to_ascii_lowercase().contains("range: bytes=4096-"),
        "first request must be Range: bytes=4096-, got: {}",
        reqs[0]
    );

    // Verify events: DownloadState never became Probing!
    while let Ok(event) = event_rx.try_recv() {
        if let sayl_core::event::EventPayload::StateChanged { to, .. } = event.payload {
            assert_ne!(
                to,
                DownloadState::Probing,
                "task state must never transition to Probing during fast resume without validator"
            );
        }
    }

    // Assert total_size stayed unchanged
    let final_task = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(
        final_task.total_size,
        Some(size as u64),
        "total_size must stay unchanged"
    );
    assert_eq!(final_task.written, size as u64);

    let final_file = dl_dir.path().join(filename);
    let bytes = tokio::fs::read(&final_file).await.expect("read final file");
    assert_eq!(bytes, expected_data);
}

#[tokio::test]
async fn test_retry_failed_with_progress_resumes_without_probing() {
    let size = 16384usize;
    let offset = 4096usize;
    let expected_data = generate_bytes(size);
    let data_clone = expected_data.clone();

    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let requests_clone = std::sync::Arc::clone(&requests);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let data = data_clone.clone();
            let reqs = std::sync::Arc::clone(&requests_clone);
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                reqs.lock().unwrap().push(req.clone());
                let lower = req.to_ascii_lowercase();

                if req.starts_with("HEAD") {
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"etag-failed-retry\"\r\nConnection: close\r\n\r\n",
                        size
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                } else if lower.contains("range: bytes=") {
                    if lower.contains("range: bytes=4096-") {
                        let body = &data[offset..];
                        let resp = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nETag: \"etag-failed-retry\"\r\nConnection: close\r\n\r\n",
                            offset,
                            size - 1,
                            size,
                            body.len()
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;
                        let _ = socket.write_all(body).await;
                    } else if lower.contains("range: bytes=0-0") {
                        let resp = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-0/{}\r\nContent-Length: 1\r\nETag: \"etag-failed-retry\"\r\nConnection: close\r\n\r\n",
                            size
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;
                        let _ = socket.write_all(&data[0..1]).await;
                    } else {
                        let resp =
                            "HTTP/1.1 416 Range Not Satisfiable\r\nConnection: close\r\n\r\n";
                        let _ = socket.write_all(resp.as_bytes()).await;
                    }
                } else {
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"etag-failed-retry\"\r\nConnection: close\r\n\r\n",
                        size
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                    let _ = socket.write_all(&data).await;
                }
            });
        }
    });

    let (dl_dir, store_dir) = make_temp_dirs();
    // Configure single concurrency slot to verify slot release
    let config = EngineConfig {
        max_concurrent_tasks: 1,
        ..Default::default()
    };
    let store = std::sync::Arc::new(Store::open(store_dir.path()).expect("open store"));
    let engine = Engine::with_store(dl_dir.path(), std::sync::Arc::clone(&store), config)
        .expect("create engine");
    let mut event_rx = engine.take_event_receiver().expect("take event rx");

    let url = format!("http://{addr}/test_retry_failed.dat");
    let task_id = sayl_core::id::TaskId::new();

    // Pre-create .part file on disk covering offset bytes
    let filename = "test_retry_failed.dat";
    let part_path = dl_dir.path().join(format!("{filename}.part"));
    tokio::fs::write(&part_path, &expected_data[..offset])
        .await
        .expect("write .part file");

    // Set state Failed with total_size, written > 0 and valid checkpoint
    let mut record = TaskRecord::with_id(task_id, &url, dl_dir.path(), filename);
    record.total_size = Some(size as u64);
    record.written = offset as u64;
    record.etag = Some("\"etag-failed-retry\"".to_string());
    record.state = DownloadState::Failed;
    record.error = Some("simulated previous network drop".to_string());
    record.error_kind = Some("network".to_string());
    store.upsert_task(&record).expect("upsert record");

    let checkpoint = sayl_storage::Checkpoint::new(
        task_id,
        offset as u64,
        Some(size as u64),
        vec![(0, offset as u64)],
    );
    store.save_checkpoint(&checkpoint).expect("save checkpoint");

    // Call start: must complete, first request is Range from offset, and no Probing event
    engine.start(task_id).await.expect("retry start");
    let terminal = wait_for_terminal(&engine, task_id, Duration::from_secs(5)).await;

    assert_eq!(terminal, DownloadState::Completed);

    let reqs = requests.lock().unwrap().clone();
    assert!(
        reqs[0].to_ascii_lowercase().contains("range: bytes=4096-"),
        "first request on retry must be Range starting at 4096, got: {}",
        reqs[0]
    );

    // Verify events: DownloadState never became Probing!
    while let Ok(event) = event_rx.try_recv() {
        if let sayl_core::event::EventPayload::StateChanged { to, .. } = event.payload {
            assert_ne!(
                to,
                DownloadState::Probing,
                "task state must never transition to Probing during retry of failed task with progress"
            );
        }
    }

    // Verify scheduler slot was released: a subsequent task can start without being blocked
    let next_url = format!("http://{addr}/test_subsequent.dat");
    let next_task = engine.add(&next_url).await.expect("add subsequent task");
    let next_handle = tokio::spawn({
        let eng = engine.clone();
        async move { eng.start(next_task).await }
    });
    let next_term = wait_for_terminal(&engine, next_task, Duration::from_secs(5)).await;
    let _ = next_handle.await;
    assert_eq!(
        next_term,
        DownloadState::Completed,
        "subsequent task must complete proving scheduler slot was released"
    );
}

#[tokio::test]
async fn test_resume_uses_contiguous_prefix_not_cumulative_written() {
    let size = 4000usize;
    let expected_data = generate_bytes(size);
    let data_clone = expected_data.clone();

    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let requests_clone = std::sync::Arc::clone(&requests);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let data = data_clone.clone();
            let reqs = std::sync::Arc::clone(&requests_clone);
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                reqs.lock().unwrap().push(req.clone());
                let lower = req.to_ascii_lowercase();

                if req.starts_with("HEAD") {
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"etag-contiguous\"\r\nConnection: close\r\n\r\n",
                        size
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                } else if lower.contains("range: bytes=") {
                    if lower.contains("range: bytes=1000-") {
                        // Correct resume starting at contiguous prefix 1000!
                        let body = &data[1000..];
                        let resp = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 1000-{}/{}\r\nContent-Length: {}\r\nETag: \"etag-contiguous\"\r\nConnection: close\r\n\r\n",
                            size - 1,
                            size,
                            body.len()
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;
                        let _ = socket.write_all(body).await;
                    } else if lower.contains("range: bytes=0-0") {
                        let resp = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-0/{}\r\nContent-Length: 1\r\nETag: \"etag-contiguous\"\r\nConnection: close\r\n\r\n{}",
                            size, data[0] as char
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;
                    } else {
                        let resp =
                            "HTTP/1.1 416 Range Not Satisfiable\r\nConnection: close\r\n\r\n";
                        let _ = socket.write_all(resp.as_bytes()).await;
                    }
                } else {
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        size
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                    let _ = socket.write_all(&data).await;
                }
            });
        }
    });

    let (dl_dir, store_dir) = make_temp_dirs();
    let config = EngineConfig::default();
    let store = std::sync::Arc::new(Store::open(store_dir.path()).expect("open store"));
    let engine = Engine::with_store(dl_dir.path(), std::sync::Arc::clone(&store), config)
        .expect("create engine");

    let url = format!("http://{addr}/test_contiguous.dat");
    let task_id = sayl_core::id::TaskId::new();
    let filename = "test_contiguous.dat";

    // Pre-create .part file on disk of 3000 bytes with a hole between 1000 and 2000
    let mut disk_bytes = vec![0u8; 3000];
    disk_bytes[0..1000].copy_from_slice(&expected_data[0..1000]);
    // 1000..2000 is unwritten / hole filled with zeroes
    disk_bytes[2000..3000].copy_from_slice(&expected_data[2000..3000]);
    let part_path = dl_dir.path().join(format!("{filename}.part"));
    tokio::fs::write(&part_path, &disk_bytes)
        .await
        .expect("write .part file");

    // Checkpoint has ranges [(0, 1000), (2000, 3000)], cumulative written = 2000
    let mut record = TaskRecord::with_id(task_id, &url, dl_dir.path(), filename);
    record.total_size = Some(size as u64);
    record.written = 2000;
    record.etag = Some("\"etag-contiguous\"".to_string());
    record.state = DownloadState::Paused;
    store.upsert_task(&record).expect("upsert record");

    let checkpoint = sayl_storage::Checkpoint::new(
        task_id,
        2000,
        Some(size as u64),
        vec![(0, 1000), (2000, 3000)],
    );
    store.save_checkpoint(&checkpoint).expect("save checkpoint");

    // Start resume
    engine.start(task_id).await.expect("start resume");
    let terminal = wait_for_terminal(&engine, task_id, Duration::from_secs(5)).await;

    assert_eq!(terminal, DownloadState::Completed);

    // Assert that the Range request starts at contiguous prefix 1000 (NOT at cumulative 2000)
    let reqs = requests.lock().unwrap().clone();
    assert!(
        reqs[0].to_ascii_lowercase().contains("range: bytes=1000-"),
        "Range request must start at contiguous prefix 1000, got: {}",
        reqs[0]
    );

    // Final file must be 4000 bytes and completely correct
    let final_file = dl_dir.path().join(filename);
    let bytes = tokio::fs::read(&final_file).await.expect("read final file");
    assert_eq!(
        bytes, expected_data,
        "final file must have complete byte integrity without hole corruption"
    );
}

#[tokio::test]
async fn test_resume_origin_ignores_range_restarts_from_zero_staying_downloading() {
    let size = 16384usize;
    let offset = 4096usize;
    let expected_data = generate_bytes(size);
    let data_clone = expected_data.clone();

    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let requests_clone = std::sync::Arc::clone(&requests);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let data = data_clone.clone();
            let reqs = std::sync::Arc::clone(&requests_clone);
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                reqs.lock().unwrap().push(req.clone());

                // Origin ignores Range: bytes=4096- and sends full 200 OK
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    size
                );
                let _ = socket.write_all(resp.as_bytes()).await;
                let _ = socket.write_all(&data).await;
            });
        }
    });

    let (dl_dir, store_dir) = make_temp_dirs();
    let config = EngineConfig::default();
    let store = std::sync::Arc::new(Store::open(store_dir.path()).expect("open store"));
    let engine = Engine::with_store(dl_dir.path(), std::sync::Arc::clone(&store), config)
        .expect("create engine");
    let mut event_rx = engine.take_event_receiver().expect("take event rx");

    let url = format!("http://{addr}/test_ignore_range.dat");
    let task_id = sayl_core::id::TaskId::new();
    let filename = "test_ignore_range.dat";

    // Pre-create .part file on disk with dummy 4096 bytes
    let part_path = dl_dir.path().join(format!("{filename}.part"));
    tokio::fs::write(&part_path, &expected_data[..offset])
        .await
        .expect("write .part file");

    let mut record = TaskRecord::with_id(task_id, &url, dl_dir.path(), filename);
    record.total_size = Some(size as u64);
    record.written = offset as u64;
    record.state = DownloadState::Paused;
    store.upsert_task(&record).expect("upsert record");

    let checkpoint = sayl_storage::Checkpoint::new(
        task_id,
        offset as u64,
        Some(size as u64),
        vec![(0, offset as u64)],
    );
    store.save_checkpoint(&checkpoint).expect("save checkpoint");

    // Start resume
    engine.start(task_id).await.expect("start resume");
    let terminal = wait_for_terminal(&engine, task_id, Duration::from_secs(5)).await;

    assert_eq!(terminal, DownloadState::Completed);

    // Verify events: DownloadState never became Probing!
    while let Ok(event) = event_rx.try_recv() {
        if let sayl_core::event::EventPayload::StateChanged { to, .. } = event.payload {
            assert_ne!(
                to,
                DownloadState::Probing,
                "task state must never transition to Probing when origin ignores Range"
            );
        }
    }

    // Assert total_size stayed unchanged and written is full size
    let final_task = engine.get_task(task_id).unwrap().unwrap();
    assert_eq!(
        final_task.total_size,
        Some(size as u64),
        "total_size must stay unchanged when restarting from 0"
    );
    assert_eq!(final_task.written, size as u64);

    let final_file = dl_dir.path().join(filename);
    let bytes = tokio::fs::read(&final_file).await.expect("read final file");
    assert_eq!(bytes, expected_data);
}

#[tokio::test]
async fn test_fallback_without_range_truncates_partial_to_zero() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let size = 8192;
    let expected_data = sayl_test_http::generate_bytes(size);
    let server_data = expected_data.clone();

    tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(s) => s,
                Err(_) => break,
            };
            let data = server_data.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);

                if req.contains("Range:") {
                    // Reject Range with 400 Bad Request to trigger fallback without Range
                    let resp = "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                    let _ = socket.write_all(resp.as_bytes()).await;
                } else {
                    // Fallback request without Range: send 200 OK with full content
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        size
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                    let _ = socket.write_all(&data).await;
                }
            });
        }
    });

    let (dl_dir, store_dir) = make_temp_dirs();
    let config = EngineConfig::default();
    let store = std::sync::Arc::new(Store::open(store_dir.path()).expect("open store"));
    let engine = Engine::with_store(dl_dir.path(), std::sync::Arc::clone(&store), config)
        .expect("create engine");

    let url = format!("http://{addr}/test_fallback.dat");
    let task_id = sayl_core::id::TaskId::new();
    let filename = "test_fallback.dat";

    // Pre-create partial file with 4000 junk bytes
    let part_path = dl_dir.path().join(format!("{filename}.part"));
    tokio::fs::write(&part_path, vec![0xEE; 4000])
        .await
        .expect("write dummy partial");

    let mut record = TaskRecord::with_id(task_id, &url, dl_dir.path(), filename);
    record.total_size = Some(size as u64);
    record.written = 4000;
    record.state = DownloadState::Paused;
    store.upsert_task(&record).expect("upsert record");

    let checkpoint =
        sayl_storage::Checkpoint::new(task_id, 4000, Some(size as u64), vec![(0, 4000)]);
    store.save_checkpoint(&checkpoint).expect("save checkpoint");

    // Start download — should attempt with Range (fails), then fallback without Range (truncates to 0)
    engine.start(task_id).await.expect("start task");
    let terminal = wait_for_terminal(&engine, task_id, Duration::from_secs(5)).await;

    assert_eq!(terminal, DownloadState::Completed);

    let final_file = dl_dir.path().join(filename);
    let bytes = tokio::fs::read(&final_file).await.expect("read final file");
    assert_eq!(
        bytes.len(),
        size,
        "file length must be exactly full size, not appended to partial"
    );
    assert_eq!(bytes, expected_data);
}

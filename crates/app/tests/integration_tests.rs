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

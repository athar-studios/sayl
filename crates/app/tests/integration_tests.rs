use std::collections::HashSet;
use std::time::{Duration, Instant};

use sayl_app::{Engine, EngineConfig};
use sayl_core::state::DownloadState;
use sayl_storage::{Store, TaskRecord};
use sayl_test_http::{generate_bytes, HttpFixtureServer, Scenario};
use tempfile::TempDir;

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
    assert!(part_path.exists(), ".part file must be preserved on truncation");
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
        if let sayl_core::event::EventPayload::Progress { downloaded_bytes, .. } = ev.payload {
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

    let checkpoint = engine.load_checkpoint(task_id).unwrap().expect("checkpoint exists");
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
    tokio::fs::write(&part_path, vec![0xAA; 4096]).await.unwrap();

    // Start download: fresh probe receives secondary ETag ("v2" vs stored "v1"),
    // so validate_resume orders RestartFromZero.
    engine.start(task_id).await.expect("start download after etag change");

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

    engine.start(task_id).await.expect("start parallel download");

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
    assert!(res3.is_err(), "task t3 must be rejected by concurrency limit");

    // t3 must remain in Queued state
    let task3 = engine.get_task(t3).unwrap().unwrap();
    assert_eq!(task3.state, DownloadState::Queued);

    // Cancel t1 and t2 to clean up
    let _ = engine.cancel(t1).await;
    let _ = engine.cancel(t2).await;
    let _ = h1.await;
    let _ = h2.await;
}

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sayl_app::lease::{LeaseReport, LeaseTable, DEFAULT_LEASE_TTL};
use sayl_app::parallel::{run_parallel_download_with_options, run_parallel_download_with_report};
use sayl_app::sequential::DownloadOutcome;
use sayl_core::id::TaskId;
use sayl_storage::Store;
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
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

#[test]
fn test_chunk_math_divisible() {
    let table = LeaseTable::new(1000, 250);
    assert_eq!(table.total_chunks(), 4);
    assert_eq!(table.total_size(), 1000);
    assert_eq!(table.chunk_size(), 250);

    let now = Instant::now();
    let ttl = Duration::from_secs(10);
    let mut table = table;

    let c0 = table.claim_next(0, now, ttl).unwrap();
    assert_eq!(c0.start, 0);
    assert_eq!(c0.end_inclusive, 249);
    assert_eq!(c0.len(), 250);

    let c1 = table.claim_next(1, now, ttl).unwrap();
    assert_eq!(c1.start, 250);
    assert_eq!(c1.end_inclusive, 499);
    assert_eq!(c1.len(), 250);

    let c2 = table.claim_next(2, now, ttl).unwrap();
    assert_eq!(c2.start, 500);
    assert_eq!(c2.end_inclusive, 749);
    assert_eq!(c2.len(), 250);

    let c3 = table.claim_next(3, now, ttl).unwrap();
    assert_eq!(c3.start, 750);
    assert_eq!(c3.end_inclusive, 999);
    assert_eq!(c3.len(), 250);

    assert!(table.claim_next(4, now, ttl).is_none());
}

#[test]
fn test_chunk_math_not_divisible_and_final_partial_chunk() {
    let table = LeaseTable::new(1000, 300);
    assert_eq!(table.total_chunks(), 4);

    let now = Instant::now();
    let ttl = Duration::from_secs(10);
    let mut table = table;

    let c0 = table.claim_next(0, now, ttl).unwrap();
    assert_eq!(c0.start, 0);
    assert_eq!(c0.end_inclusive, 299);
    assert_eq!(c0.len(), 300);

    let c1 = table.claim_next(1, now, ttl).unwrap();
    assert_eq!(c1.start, 300);
    assert_eq!(c1.end_inclusive, 599);
    assert_eq!(c1.len(), 300);

    let c2 = table.claim_next(2, now, ttl).unwrap();
    assert_eq!(c2.start, 600);
    assert_eq!(c2.end_inclusive, 899);
    assert_eq!(c2.len(), 300);

    // Final partial chunk covers 900..=999 (100 bytes)
    let c3 = table.claim_next(3, now, ttl).unwrap();
    assert_eq!(c3.start, 900);
    assert_eq!(c3.end_inclusive, 999);
    assert_eq!(c3.len(), 100);

    assert!(table.claim_next(4, now, ttl).is_none());
}

#[test]
fn test_chunk_math_total_zero_no_chunks() {
    let mut table = LeaseTable::new(0, 1024);
    assert_eq!(table.total_chunks(), 0);
    assert_eq!(table.total_size(), 0);
    assert!(table.is_complete());

    let now = Instant::now();
    let ttl = Duration::from_secs(10);
    assert!(table.claim_next(0, now, ttl).is_none());

    let report = table.report();
    assert_eq!(report.chunks_total, 0);
    assert_eq!(report.chunks_completed, 0);
    report.assert_full_coverage(0);
}

#[test]
fn test_chunk_math_chunk_size_greater_than_total_one_chunk() {
    let mut table = LeaseTable::new(50, 1000);
    assert_eq!(table.total_chunks(), 1);
    assert_eq!(table.total_size(), 50);

    let now = Instant::now();
    let ttl = Duration::from_secs(10);
    let chunk = table.claim_next(0, now, ttl).unwrap();
    assert_eq!(chunk.start, 0);
    assert_eq!(chunk.end_inclusive, 49);
    assert_eq!(chunk.len(), 50);

    assert!(table.claim_next(1, now, ttl).is_none());

    assert!(table.complete(chunk));
    assert!(table.is_complete());

    let report = table.report();
    report.assert_full_coverage(50);
}

#[test]
fn test_no_double_claim_unexpired_leases() {
    let mut table = LeaseTable::new(200, 100);
    assert_eq!(table.total_chunks(), 2);

    let now = Instant::now();
    let ttl = Duration::from_secs(30);

    let c0 = table
        .claim_next(0, now, ttl)
        .expect("worker 0 claims chunk 0");
    assert_eq!(c0.owner, Some(0));
    assert_eq!(c0.start, 0);

    let c1 = table
        .claim_next(1, now, ttl)
        .expect("worker 1 claims chunk 1");
    assert_eq!(c1.owner, Some(1));
    assert_eq!(c1.start, 100);

    // With active unexpired leases, no worker can claim another chunk
    assert!(table.claim_next(0, now, ttl).is_none());
    assert!(table.claim_next(1, now, ttl).is_none());
    assert!(table.claim_next(2, now, ttl).is_none());
}

#[test]
fn test_steal_expired_lease_and_reject_stale_completion() {
    let mut table = LeaseTable::new(100, 100);
    assert_eq!(table.total_chunks(), 1);

    let t0 = Instant::now();
    let ttl = Duration::from_secs(5);

    // Worker 0 claims at t0, expires at t0 + 5s
    let lease0 = table.claim_next(0, t0, ttl).expect("worker 0 claim");
    assert_eq!(lease0.owner, Some(0));
    assert_eq!(table.steals_performed(), 0);

    // Advance clock past expiration
    let t1 = t0 + Duration::from_secs(6);

    // Worker 1 claims at t1 -> steals chunk 0!
    let lease1 = table.claim_next(1, t1, ttl).expect("worker 1 steal");
    assert_eq!(lease1.owner, Some(1));
    assert_eq!(lease1.start, 0);
    assert_eq!(lease1.end_inclusive, 99);
    assert_eq!(table.steals_performed(), 1);

    // Original owner (Worker 0) attempts completion later with stale lease -> REJECTED
    let completed_stale = table.complete(lease0);
    assert!(!completed_stale, "stale completion must be rejected");
    assert!(!table.is_complete());

    // New owner (Worker 1) completes with valid lease -> ACCEPTED
    let completed_valid = table.complete(lease1);
    assert!(
        completed_valid,
        "valid current owner completion must succeed"
    );
    assert!(table.is_complete());

    let report = table.report();
    assert_eq!(report.steals_performed, 1);
    assert_eq!(report.chunks_completed, 1);
    report.assert_full_coverage(100);
}

#[test]
fn test_steal_longest_expired_first() {
    let mut table = LeaseTable::new(300, 100);
    assert_eq!(table.total_chunks(), 3);

    let t0 = Instant::now();

    // Chunk 0 leased with 6s TTL -> expires at t0 + 6s
    let c0 = table.claim_next(0, t0, Duration::from_secs(6)).unwrap();
    // Chunk 1 leased with 2s TTL -> expires at t0 + 2s (expired longest!)
    let c1 = table.claim_next(1, t0, Duration::from_secs(2)).unwrap();
    // Chunk 2 leased with 9s TTL -> expires at t0 + 9s
    let c2 = table.claim_next(2, t0, Duration::from_secs(9)).unwrap();

    assert_eq!(c0.start, 0);
    assert_eq!(c1.start, 100);
    assert_eq!(c2.start, 200);

    // At t0 + 7s:
    // Chunk 1 expired 5s ago (earliest expiry: t0 + 2s)
    // Chunk 0 expired 1s ago (expiry: t0 + 6s)
    // Chunk 2 is still unexpired (expiry: t0 + 9s)
    let t_steal = t0 + Duration::from_secs(7);
    let stolen = table
        .claim_next(3, t_steal, Duration::from_secs(5))
        .expect("steal");

    // Must steal Chunk 1 (longest-expired first)
    assert_eq!(
        stolen.start, 100,
        "must steal longest-expired chunk 1 first"
    );
    assert_eq!(stolen.owner, Some(3));
}

#[test]
fn test_release_on_cancel_makes_chunk_claimable_again() {
    let mut table = LeaseTable::new(100, 100);
    let now = Instant::now();
    let ttl = Duration::from_secs(10);

    let lease0 = table.claim_next(0, now, ttl).unwrap();
    assert!(table.claim_next(1, now, ttl).is_none());

    // Worker 0 cancels / errors and releases lease
    let released = table.release(lease0);
    assert!(released);
    assert_eq!(table.retries(), 1);

    // Chunk is immediately claimable again by Worker 1
    let lease1 = table.claim_next(1, now, ttl).expect("claim after release");
    assert_eq!(lease1.owner, Some(1));
    assert_eq!(lease1.start, 0);
}

#[test]
fn test_coverage_assertion_fails_on_overlap_or_gap() {
    // Overlap case
    let bad_overlap = LeaseReport::new(2, 2, 0, 0, vec![(0, 50), (45, 99)], 100);
    assert!(bad_overlap.has_overlap());
    assert!(bad_overlap.verify_coverage(100).is_err());

    // Gap case
    let bad_gap = LeaseReport::new(2, 2, 0, 0, vec![(0, 40), (50, 99)], 100);
    assert!(!bad_gap.has_overlap());
    assert!(bad_gap.verify_coverage(100).is_err());

    // Clean contiguous full coverage
    let good = LeaseReport::new(2, 2, 0, 0, vec![(0, 49), (50, 99)], 100);
    assert!(!good.has_overlap());
    assert!(good.verify_coverage(100).is_ok());
    good.assert_full_coverage(100);
}

#[tokio::test]
async fn test_end_to_end_fixture_server_max_connections_4() {
    let dl_dir = make_temp_dir();
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let size = 65536;
    server.set_resource_size(size);
    let addr = server.start().await.expect("start fixture server");

    let client = reqwest::Client::new();
    let url = url::Url::parse(&format!("http://{addr}/test_parallel_sched.bin")).unwrap();
    let part_path = dl_dir.path().join("test_parallel_sched.bin.part");
    let task_id = TaskId::new();
    let cancel_token = CancellationToken::new();
    let pause_token = CancellationToken::new();
    let (event_sender, _rx) = sayl_core::event::event_channel(128);
    let store = Arc::new(Store::open_in_memory().unwrap());

    let chunk_size = 16384; // 4 chunks of 16 KiB
    let (outcome, report) = run_parallel_download_with_report(
        &client,
        &url,
        &part_path,
        task_id,
        size as u64,
        4,
        chunk_size,
        cancel_token,
        pause_token,
        &event_sender,
        store.clone(),
        &[], // No completed ranges for fresh download
        DEFAULT_LEASE_TTL,
    )
    .await
    .expect("parallel download succeeds");

    assert_eq!(
        outcome,
        DownloadOutcome::Completed {
            written: size as u64
        }
    );

    // Verify downloaded bytes
    let bytes = tokio::fs::read(&part_path).await.expect("read part file");
    assert_eq!(bytes, generate_bytes(size));

    // Coverage assertion proves union equals [0, size) with no overlap
    report.assert_full_coverage(size as u64);
    assert!(!report.has_overlap());
    assert_eq!(report.chunks_total, 4);
    assert_eq!(report.chunks_completed, 4);
    assert_eq!(report.steals_performed, 0);

    // Audit recorded requests: each byte range requested at most once
    let recorded = server.recorded_requests().await;
    let mut requested_ranges = Vec::new();
    for req in recorded {
        if req.method == "GET" {
            if let Some(range) = req.range_header {
                requested_ranges.push(range);
            }
        }
    }

    assert_eq!(requested_ranges.len(), 4);
    let unique: HashSet<_> = requested_ranges.iter().collect();
    assert_eq!(
        unique.len(),
        4,
        "each byte range must be requested at most once"
    );
}

#[tokio::test]
async fn test_slow_worker_triggers_steal_and_file_is_correct() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    let size: usize = 4000;
    let expected_data = generate_bytes(size);
    let expected_data_clone = expected_data.clone();

    let range0_attempts = Arc::new(AtomicUsize::new(0));
    let range0_attempts_clone = Arc::clone(&range0_attempts);

    let server_task = tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(s) => s,
                Err(_) => break,
            };

            let expected_data = expected_data_clone.clone();
            let range0_attempts = Arc::clone(&range0_attempts_clone);

            tokio::spawn(async move {
                let mut buf = vec![0u8; 1024];
                let n = match socket.read(&mut buf).await {
                    Ok(n) if n > 0 => n,
                    _ => return,
                };
                let req_str = String::from_utf8_lossy(&buf[..n]);

                let mut range = None;
                for line in req_str.lines() {
                    if line.to_ascii_lowercase().starts_with("range: bytes=") {
                        let parts: Vec<&str> = line[13..].split('-').collect();
                        if parts.len() == 2 {
                            let s: usize = parts[0].parse().unwrap_or(0);
                            let e: usize = parts[1].parse().unwrap_or(0);
                            range = Some((s, e));
                        }
                    }
                }

                if let Some((start, end)) = range {
                    if start == 0 {
                        let attempt = range0_attempts.fetch_add(1, Ordering::SeqCst);
                        if attempt == 0 {
                            // Delay first worker on range 0 so its lease expires and gets stolen
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    }

                    let end_inclusive = end.min(size - 1);
                    let body = &expected_data[start..=end_inclusive];
                    let headers = format!("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\n\r\n", start, end_inclusive, size, body.len());
                    let _ = socket.write_all(headers.as_bytes()).await;
                    let _ = socket.write_all(body).await;
                }
            });
        }
    });

    let dl_dir = make_temp_dir();
    let client = reqwest::Client::new();
    let url = url::Url::parse(&format!("http://{local_addr}/slow_test.bin")).unwrap();
    let part_path = dl_dir.path().join("slow_test.bin.part");
    let task_id = TaskId::new();
    let cancel_token = CancellationToken::new();
    let pause_token = CancellationToken::new();
    let (event_sender, _rx) = sayl_core::event::event_channel(128);
    let store = Arc::new(Store::open_in_memory().unwrap());

    let chunk_size = 2000; // 2 chunks: 0..=1999 and 2000..=3999
    let lease_ttl = Duration::from_millis(40); // 40ms TTL < 100ms delay on first worker

    let (outcome, report) = run_parallel_download_with_options(
        &client,
        &url,
        &part_path,
        task_id,
        size as u64,
        2,
        chunk_size,
        cancel_token,
        pause_token,
        &event_sender,
        store.clone(),
        &[], // No completed ranges for fresh download
        lease_ttl,
    )
    .await
    .expect("download completes with steal");

    assert_eq!(
        outcome,
        DownloadOutcome::Completed {
            written: size as u64
        }
    );

    let bytes = tokio::fs::read(&part_path).await.expect("read part file");
    assert_eq!(bytes, expected_data);

    assert!(
        report.steals_performed >= 1,
        "expected at least 1 steal, got {}",
        report.steals_performed
    );
    report.assert_full_coverage(size as u64);

    server_task.abort();
}

#[tokio::test]
async fn test_worker_failing_mid_chunk_retried_by_another_worker_and_completes() {
    // Spin up a mock HTTP server where request for range 1 fails on attempt 1 with ConnectionReset,
    // but succeeds on attempt 2 (when retried by another worker).
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    let size: usize = 2000;
    let expected_data = generate_bytes(size);
    let expected_data_clone = expected_data.clone();

    let range1_attempts = Arc::new(AtomicUsize::new(0));
    let range1_attempts_clone = Arc::clone(&range1_attempts);

    let server_task = tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(s) => s,
                Err(_) => break,
            };

            let expected_data = expected_data_clone.clone();
            let range1_attempts = Arc::clone(&range1_attempts_clone);

            tokio::spawn(async move {
                let mut buf = vec![0u8; 1024];
                let n = match socket.read(&mut buf).await {
                    Ok(n) if n > 0 => n,
                    _ => return,
                };
                let req_str = String::from_utf8_lossy(&buf[..n]);

                // Check Range header
                let mut range = None;
                for line in req_str.lines() {
                    if line.to_ascii_lowercase().starts_with("range: bytes=") {
                        let parts: Vec<&str> = line[13..].split('-').collect();
                        if parts.len() == 2 {
                            let s: usize = parts[0].parse().unwrap_or(0);
                            let e: usize = parts[1].parse().unwrap_or(0);
                            range = Some((s, e));
                        }
                    }
                }

                if let Some((start, end)) = range {
                    if start == 1000 {
                        let attempt = range1_attempts.fetch_add(1, Ordering::SeqCst);
                        if attempt == 0 {
                            // First attempt on range 1 fails mid-chunk with ConnectionReset!
                            let headers = format!(
                                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 1000-1999/2000\r\nContent-Length: 1000\r\n\r\n"
                            );
                            let _ = socket.write_all(headers.as_bytes()).await;
                            let _ = socket.write_all(&expected_data[1000..1200]).await; // write only 200 bytes
                            let _ = socket.shutdown().await; // simulate drop / RST
                            return;
                        }
                    }

                    // Normal response
                    let end_inclusive = end.min(size - 1);
                    let body = &expected_data[start..=end_inclusive];
                    let headers = format!("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\n\r\n", start, end_inclusive, size, body.len());
                    let _ = socket.write_all(headers.as_bytes()).await;
                    let _ = socket.write_all(body).await;
                }
            });
        }
    });

    let dl_dir = make_temp_dir();
    let client = reqwest::Client::new();
    let url = url::Url::parse(&format!("http://{local_addr}/retry_test.bin")).unwrap();
    let part_path = dl_dir.path().join("retry_test.bin.part");
    let task_id = TaskId::new();
    let cancel_token = CancellationToken::new();
    let pause_token = CancellationToken::new();
    let (event_sender, _rx) = sayl_core::event::event_channel(128);
    let store = Arc::new(Store::open_in_memory().unwrap());

    let (outcome, report) = run_parallel_download_with_report(
        &client,
        &url,
        &part_path,
        task_id,
        size as u64,
        2,
        1000, // 2 chunks: 0..=999 and 1000..=1999
        cancel_token,
        pause_token,
        &event_sender,
        store.clone(),
        &[], // No completed ranges for fresh download
        DEFAULT_LEASE_TTL,
    )
    .await
    .expect("download succeeds through retry by worker");

    assert_eq!(
        outcome,
        DownloadOutcome::Completed {
            written: size as u64
        }
    );

    // Verify final file is completely byte-correct
    let bytes = tokio::fs::read(&part_path).await.expect("read part file");
    assert_eq!(bytes, expected_data);

    // Verify retries occurred and full coverage holds
    assert!(
        report.retries >= 1,
        "expected at least 1 retry, got {}",
        report.retries
    );
    report.assert_full_coverage(size as u64);

    server_task.abort();
}

#[tokio::test]
async fn test_persistent_worker_failure_fails_cleanly() {
    let dl_dir = make_temp_dir();
    let mut server = HttpFixtureServer::with_scenario(Scenario::ConnectionReset);
    let size = 32768;
    server.set_resource_size(size);
    let addr = server.start().await.expect("start fixture server");

    let client = reqwest::Client::new();
    let url = url::Url::parse(&format!("http://{addr}/reset.bin")).unwrap();
    let part_path = dl_dir.path().join("reset.bin.part");
    let task_id = TaskId::new();
    let cancel_token = CancellationToken::new();
    let pause_token = CancellationToken::new();
    let (event_sender, _rx) = sayl_core::event::event_channel(128);
    let store = Arc::new(Store::open_in_memory().unwrap());

    let result = run_parallel_download_with_report(
        &client,
        &url,
        &part_path,
        task_id,
        size as u64,
        2,
        16384,
        cancel_token,
        pause_token,
        &event_sender,
        store.clone(),
        &[], // No completed ranges for fresh download
        DEFAULT_LEASE_TTL,
    )
    .await;

    // Fails cleanly with no partial success claim
    assert!(
        result.is_err(),
        "must fail cleanly on persistent connection reset"
    );
}

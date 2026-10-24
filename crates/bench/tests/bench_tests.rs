use sayl_bench::run_all_benchmarks;
use serde_json::Value;

#[tokio::test]
async fn test_smoke_1mib_measurement() {
    let size = 1024 * 1024; // 1 MiB
    let report = run_all_benchmarks(size, Some("all"))
        .await
        .expect("Benchmark run must succeed");

    assert_eq!(report.file_size_bytes, size);

    // Assert sequential bytes transferred
    let seq = report.sequential.as_ref().expect("sequential should be present");
    assert_eq!(seq.bytes_transferred, size);
    assert_eq!(seq.requests_issued, 1);
    assert!(seq.wall_time_secs > 0.0);
    assert!(seq.throughput_mb_s > 0.0);

    // Assert parallel bytes transferred
    let p4 = report.parallel_4.as_ref().expect("parallel_4 should be present");
    assert_eq!(p4.bytes_transferred, size);
    assert_eq!(p4.connections, 4);
    assert_eq!(p4.requests_issued, 4);

    let p8 = report.parallel_8.as_ref().expect("parallel_8 should be present");
    assert_eq!(p8.bytes_transferred, size);
    assert_eq!(p8.connections, 8);
    assert_eq!(p8.requests_issued, 8);

    // Assert range overhead
    let ro = report.range_overhead.as_ref().expect("range_overhead should be present");
    assert_eq!(ro.file_size_bytes, size);
    assert_eq!(ro.bytes_transferred, size);
    assert_eq!(ro.requests_issued, 4);
    assert!((ro.transfer_ratio - 1.0).abs() < 0.0001);

    // Assert resume cost
    let res = report.resume.as_ref().expect("resume should be present");
    assert_eq!(res.interrupted_bytes, size / 2);
    assert_eq!(res.resumed_bytes, size - (size / 2));
    assert_eq!(res.total_bytes_transferred, size);
    assert_eq!(res.re_fetched_bytes, 0);
    assert_eq!(res.re_fetch_ratio, 0.0);
    assert_eq!(res.requests_issued, 2);

    // Assert memory measurement
    let mem = report.memory.as_ref().expect("memory should be present");
    #[cfg(windows)]
    {
        assert!(mem.peak_rss_bytes.is_some());
        assert!(mem.peak_rss_bytes.unwrap() > 0);
        assert!(mem.peak_rss_mb.is_some());
    }
}

#[tokio::test]
async fn test_json_output_fields() {
    let size = 512 * 1024; // 512 KiB
    let report = run_all_benchmarks(size, Some("all"))
        .await
        .expect("Benchmark run must succeed");

    let json_str = report.to_json();
    let v: Value = serde_json::from_str(&json_str).expect("JSON must be valid");

    // Check system_info
    assert!(v["system_info"]["os"].is_string());
    assert!(v["system_info"]["arch"].is_string());
    assert!(v["system_info"]["cpu_count"].is_number());
    assert!(v["system_info"]["build_profile"].is_string());

    // Check file_size_bytes
    assert_eq!(v["file_size_bytes"], size);

    // Check sequential
    assert_eq!(v["sequential"]["bytes_transferred"], size);
    assert!(v["sequential"]["wall_time_secs"].is_number());
    assert!(v["sequential"]["throughput_mb_s"].is_number());
    assert_eq!(v["sequential"]["requests_issued"], 1);

    // Check parallel_4
    assert_eq!(v["parallel_4"]["connections"], 4);
    assert_eq!(v["parallel_4"]["bytes_transferred"], size);
    assert!(v["parallel_4"]["wall_time_secs"].is_number());
    assert!(v["parallel_4"]["throughput_mb_s"].is_number());
    assert!(v["parallel_4"]["speedup_ratio"].is_number());
    assert_eq!(v["parallel_4"]["requests_issued"], 4);

    // Check parallel_8
    assert_eq!(v["parallel_8"]["connections"], 8);
    assert_eq!(v["parallel_8"]["bytes_transferred"], size);
    assert!(v["parallel_8"]["wall_time_secs"].is_number());
    assert!(v["parallel_8"]["throughput_mb_s"].is_number());
    assert!(v["parallel_8"]["speedup_ratio"].is_number());
    assert_eq!(v["parallel_8"]["requests_issued"], 8);

    // Check range_overhead
    assert_eq!(v["range_overhead"]["file_size_bytes"], size);
    assert_eq!(v["range_overhead"]["bytes_transferred"], size);
    assert_eq!(v["range_overhead"]["requests_issued"], 4);
    assert_eq!(v["range_overhead"]["transfer_ratio"], 1.0);

    // Check resume
    assert_eq!(v["resume"]["interrupted_bytes"], size / 2);
    assert_eq!(v["resume"]["resumed_bytes"], size - (size / 2));
    assert_eq!(v["resume"]["total_bytes_transferred"], size);
    assert_eq!(v["resume"]["re_fetched_bytes"], 0);
    assert_eq!(v["resume"]["re_fetch_ratio"], 0.0);
    assert_eq!(v["resume"]["requests_issued"], 2);

    // Check memory
    assert!(v["memory"].is_object());
}

use std::time::Instant;
use futures_util::StreamExt;
use sayl_http::{build_client, ClientConfig};
use sayl_test_http::{HttpFixtureServer, Scenario};
use serde::{Deserialize, Serialize};

#[cfg(windows)]
pub fn get_peak_rss_bytes() -> Option<u64> {
    #[repr(C)]
    struct PROCESS_MEMORY_COUNTERS {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(
            process: isize,
            ppsmc: *mut PROCESS_MEMORY_COUNTERS,
            cb: u32,
        ) -> i32;
    }
    unsafe {
        let mut counters = std::mem::zeroed::<PROCESS_MEMORY_COUNTERS>();
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let handle = GetCurrentProcess();
        if K32GetProcessMemoryInfo(handle, &mut counters, counters.cb) != 0 {
            Some(counters.peak_working_set_size as u64)
        } else {
            None
        }
    }
}

#[cfg(not(windows))]
pub fn get_peak_rss_bytes() -> Option<u64> {
    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub cpu_count: usize,
    pub build_profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequentialResult {
    pub bytes_transferred: usize,
    pub wall_time_secs: f64,
    pub throughput_mb_s: f64,
    pub requests_issued: usize,
    pub peak_rss_bytes: Option<u64>,
    pub peak_rss_mb: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelResult {
    pub connections: usize,
    pub bytes_transferred: usize,
    pub wall_time_secs: f64,
    pub throughput_mb_s: f64,
    pub speedup_ratio: f64,
    pub requests_issued: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeOverheadResult {
    pub file_size_bytes: usize,
    pub bytes_transferred: usize,
    pub requests_issued: usize,
    pub transfer_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeResult {
    pub interrupted_bytes: usize,
    pub resumed_bytes: usize,
    pub total_bytes_transferred: usize,
    pub re_fetched_bytes: usize,
    pub re_fetch_ratio: f64,
    pub requests_issued: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryResult {
    pub peak_rss_bytes: Option<u64>,
    pub peak_rss_mb: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub system_info: SystemInfo,
    pub file_size_bytes: usize,
    pub sequential: Option<SequentialResult>,
    pub parallel_4: Option<ParallelResult>,
    pub parallel_8: Option<ParallelResult>,
    pub range_overhead: Option<RangeOverheadResult>,
    pub resume: Option<ResumeResult>,
    pub memory: Option<MemoryResult>,
}

pub fn current_system_info() -> SystemInfo {
    let profile = if cfg!(debug_assertions) {
        "debug (GNU + rust-lld)"
    } else {
        "release (GNU + rust-lld)"
    };
    SystemInfo {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        cpu_count: std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1),
        build_profile: profile.to_string(),
    }
}

pub async fn run_sequential(
    client: &reqwest::Client,
    server_url: &str,
    size: usize,
) -> Result<SequentialResult, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("{}/sequential.bin", server_url);
    let start = Instant::now();
    let res = client.get(&url).send().await?;
    if !res.status().is_success() {
        return Err(format!("Sequential download failed with status {}", res.status()).into());
    }

    let mut stream = res.bytes_stream();
    let mut total_bytes = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        total_bytes += chunk.len();
    }
    let elapsed = start.elapsed();

    if total_bytes != size {
        return Err(format!(
            "Sequential download size mismatch: expected {} bytes, got {}",
            size, total_bytes
        )
        .into());
    }

    let wall_time_secs = elapsed.as_secs_f64();
    let throughput_mb_s = if wall_time_secs > 0.0 {
        (total_bytes as f64 / (1024.0 * 1024.0)) / wall_time_secs
    } else {
        0.0
    };

    let peak_rss_bytes = get_peak_rss_bytes();
    let peak_rss_mb = peak_rss_bytes.map(|b| b as f64 / (1024.0 * 1024.0));

    Ok(SequentialResult {
        bytes_transferred: total_bytes,
        wall_time_secs,
        throughput_mb_s,
        requests_issued: 1,
        peak_rss_bytes,
        peak_rss_mb,
    })
}

pub async fn run_parallel(
    client: &reqwest::Client,
    server_url: &str,
    size: usize,
    connections: usize,
    seq_throughput: Option<f64>,
) -> Result<ParallelResult, Box<dyn std::error::Error + Send + Sync>> {
    if connections == 0 {
        return Err("Connections must be greater than 0".into());
    }

    let chunk_size = size / connections;
    let start = Instant::now();
    let mut handles = Vec::new();

    for i in 0..connections {
        let client = client.clone();
        let start_byte = i * chunk_size;
        let end_byte = if i == connections - 1 {
            size - 1
        } else {
            (i + 1) * chunk_size - 1
        };
        let expected_part_len = end_byte - start_byte + 1;
        let url = format!("{}/parallel_{}_{}.bin", server_url, connections, i);

        handles.push(tokio::spawn(async move {
            let range_header = format!("bytes={}-{}", start_byte, end_byte);
            let res = client
                .get(url)
                .header("Range", range_header)
                .send()
                .await
                .map_err(|e| e.to_string())?;

            if res.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                return Err(format!(
                    "Connection {} expected 206 Partial Content, got {}",
                    i,
                    res.status()
                ));
            }

            let mut stream = res.bytes_stream();
            let mut part_bytes = 0;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| e.to_string())?;
                part_bytes += chunk.len();
            }

            if part_bytes != expected_part_len {
                return Err(format!(
                    "Connection {} part length mismatch: expected {}, got {}",
                    i, expected_part_len, part_bytes
                ));
            }

            Ok::<usize, String>(part_bytes)
        }));
    }

    let mut total_bytes = 0;
    for handle in handles {
        let part_bytes = handle
            .await
            .map_err(|e| format!("Parallel task panicked: {}", e))??;
        total_bytes += part_bytes;
    }
    let elapsed = start.elapsed();

    if total_bytes != size {
        return Err(format!(
            "Parallel download size mismatch: expected {} bytes, got {}",
            size, total_bytes
        )
        .into());
    }

    let wall_time_secs = elapsed.as_secs_f64();
    let throughput_mb_s = if wall_time_secs > 0.0 {
        (total_bytes as f64 / (1024.0 * 1024.0)) / wall_time_secs
    } else {
        0.0
    };

    let speedup_ratio = match seq_throughput {
        Some(seq_tp) if seq_tp > 0.0 => throughput_mb_s / seq_tp,
        _ => 1.0,
    };

    Ok(ParallelResult {
        connections,
        bytes_transferred: total_bytes,
        wall_time_secs,
        throughput_mb_s,
        speedup_ratio,
        requests_issued: connections,
    })
}

pub async fn run_range_overhead(
    client: &reqwest::Client,
    server_url: &str,
    size: usize,
    connections: usize,
) -> Result<RangeOverheadResult, Box<dyn std::error::Error + Send + Sync>> {
    let connections = connections.max(1);
    let chunk_size = size / connections;
    let mut total_transferred = 0;

    for i in 0..connections {
        let start_byte = i * chunk_size;
        let end_byte = if i == connections - 1 {
            size - 1
        } else {
            (i + 1) * chunk_size - 1
        };
        let expected_part_len = end_byte - start_byte + 1;
        let url = format!("{}/range_overhead_{}.bin", server_url, i);
        let range_header = format!("bytes={}-{}", start_byte, end_byte);

        let res = client
            .get(&url)
            .header("Range", range_header)
            .send()
            .await?;

        if res.status() != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(format!(
                "Range request {} failed with status {}",
                i,
                res.status()
            )
            .into());
        }

        let mut stream = res.bytes_stream();
        let mut part_bytes = 0;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            part_bytes += chunk.len();
        }

        if part_bytes != expected_part_len {
            return Err(format!(
                "Range part {} length mismatch: expected {}, got {}",
                i, expected_part_len, part_bytes
            )
            .into());
        }
        total_transferred += part_bytes;
    }

    let transfer_ratio = if size > 0 {
        total_transferred as f64 / size as f64
    } else {
        1.0
    };

    Ok(RangeOverheadResult {
        file_size_bytes: size,
        bytes_transferred: total_transferred,
        requests_issued: connections,
        transfer_ratio,
    })
}

pub async fn run_resume(
    client: &reqwest::Client,
    server_url: &str,
    size: usize,
) -> Result<ResumeResult, Box<dyn std::error::Error + Send + Sync>> {
    let half_point = size / 2;
    let url = format!("{}/resume.bin", server_url);

    // Phase 1: Stream until 50% reached, then interrupt (cancel/drop)
    let res = client.get(&url).send().await?;
    if !res.status().is_success() {
        return Err(format!("Resume phase 1 failed with status {}", res.status()).into());
    }

    let mut stream = res.bytes_stream();
    let mut interrupted_bytes = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let need = half_point.saturating_sub(interrupted_bytes);
        if chunk.len() >= need {
            interrupted_bytes += need;
            break; // Drop stream here, simulating interrupt at exactly 50%
        } else {
            interrupted_bytes += chunk.len();
        }
    }
    drop(stream);

    if interrupted_bytes != half_point {
        return Err(format!(
            "Interrupt failed: expected {} bytes at 50%, got {}",
            half_point, interrupted_bytes
        )
        .into());
    }

    // Phase 2: Resume from interrupted byte offset
    let resume_range = format!("bytes={}-{}", interrupted_bytes, size - 1);
    let expected_resume_len = size - interrupted_bytes;
    let res2 = client
        .get(&url)
        .header("Range", resume_range)
        .send()
        .await?;

    if res2.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(format!("Resume phase 2 failed with status {}", res2.status()).into());
    }

    let mut stream2 = res2.bytes_stream();
    let mut resumed_bytes = 0;
    while let Some(chunk) = stream2.next().await {
        let chunk = chunk?;
        resumed_bytes += chunk.len();
    }

    if resumed_bytes != expected_resume_len {
        return Err(format!(
            "Resume phase 2 length mismatch: expected {}, got {}",
            expected_resume_len, resumed_bytes
        )
        .into());
    }

    let total_bytes_transferred = interrupted_bytes + resumed_bytes;
    let re_fetched_bytes = total_bytes_transferred.saturating_sub(size);
    let re_fetch_ratio = if size > 0 {
        re_fetched_bytes as f64 / size as f64
    } else {
        0.0
    };

    Ok(ResumeResult {
        interrupted_bytes,
        resumed_bytes,
        total_bytes_transferred,
        re_fetched_bytes,
        re_fetch_ratio,
        requests_issued: 2,
    })
}

pub async fn run_all_benchmarks(
    size: usize,
    scenario_filter: Option<&str>,
) -> Result<BenchmarkReport, Box<dyn std::error::Error + Send + Sync>> {
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    server.set_resource_size(size);
    let addr = server.start().await?;
    let server_url = format!("http://{}", addr);

    let client = build_client(&ClientConfig::default())?;

    let filter = scenario_filter.unwrap_or("all").to_lowercase();
    let run_all = filter == "all" || filter.is_empty();

    let mut report = BenchmarkReport {
        system_info: current_system_info(),
        file_size_bytes: size,
        sequential: None,
        parallel_4: None,
        parallel_8: None,
        range_overhead: None,
        resume: None,
        memory: None,
    };

    let mut seq_throughput = None;

    if run_all || filter == "sequential" || filter == "parallel" || filter == "parallel_4" || filter == "parallel_8" {
        let seq = run_sequential(&client, &server_url, size).await?;
        seq_throughput = Some(seq.throughput_mb_s);
        if run_all || filter == "sequential" {
            report.sequential = Some(seq);
        }
    }

    if run_all || filter == "parallel" || filter == "parallel_4" {
        let p4 = run_parallel(&client, &server_url, size, 4, seq_throughput).await?;
        report.parallel_4 = Some(p4);
    }

    if run_all || filter == "parallel" || filter == "parallel_8" {
        let p8 = run_parallel(&client, &server_url, size, 8, seq_throughput).await?;
        report.parallel_8 = Some(p8);
    }

    if run_all || filter == "range" || filter == "range_overhead" {
        let ro = run_range_overhead(&client, &server_url, size, 4).await?;
        report.range_overhead = Some(ro);
    }

    if run_all || filter == "resume" {
        let res = run_resume(&client, &server_url, size).await?;
        report.resume = Some(res);
    }

    if run_all || filter == "memory" {
        let peak_rss = get_peak_rss_bytes();
        report.memory = Some(MemoryResult {
            peak_rss_bytes: peak_rss,
            peak_rss_mb: peak_rss.map(|b| b as f64 / (1024.0 * 1024.0)),
        });
    }

    server.shutdown();
    Ok(report)
}

impl BenchmarkReport {
    pub fn to_table(&self) -> String {
        let mut out = String::new();
        out.push_str("========================================================================================\n");
        out.push_str("                       SAYL PERFORMANCE BENCHMARK (PERF-001)                           \n");
        out.push_str("========================================================================================\n");
        out.push_str(&format!(
            " OS: {} | Arch: {} | Cores: {} | Profile: {}\n",
            self.system_info.os,
            self.system_info.arch,
            self.system_info.cpu_count,
            self.system_info.build_profile
        ));
        out.push_str(&format!(
            " Fixture Size: {} bytes ({:.2} MiB)\n",
            self.file_size_bytes,
            self.file_size_bytes as f64 / (1024.0 * 1024.0)
        ));
        out.push_str("----------------------------------------------------------------------------------------\n");
        out.push_str(&format!(
            "{:<24} | {:<12} | {:<12} | {:<12} | {:<14}\n",
            "Benchmark Scenario", "Transferred", "Wall Time", "Throughput", "Details / Ratio"
        ));
        out.push_str("----------------------------------------------------------------------------------------\n");

        if let Some(s) = &self.sequential {
            out.push_str(&format!(
                "{:<24} | {:<12} | {:<10.3} s | {:<9.2} MB/s | requests: {}\n",
                "1. Sequential (1 conn)",
                format!("{:.2} MiB", s.bytes_transferred as f64 / (1024.0 * 1024.0)),
                s.wall_time_secs,
                s.throughput_mb_s,
                s.requests_issued
            ));
        }

        if let Some(p4) = &self.parallel_4 {
            out.push_str(&format!(
                "{:<24} | {:<12} | {:<10.3} s | {:<9.2} MB/s | speedup: {:.2}x\n",
                "2a. Parallel (4 conns)",
                format!("{:.2} MiB", p4.bytes_transferred as f64 / (1024.0 * 1024.0)),
                p4.wall_time_secs,
                p4.throughput_mb_s,
                p4.speedup_ratio
            ));
        }

        if let Some(p8) = &self.parallel_8 {
            out.push_str(&format!(
                "{:<24} | {:<12} | {:<10.3} s | {:<9.2} MB/s | speedup: {:.2}x\n",
                "2b. Parallel (8 conns)",
                format!("{:.2} MiB", p8.bytes_transferred as f64 / (1024.0 * 1024.0)),
                p8.wall_time_secs,
                p8.throughput_mb_s,
                p8.speedup_ratio
            ));
        }

        if let Some(ro) = &self.range_overhead {
            out.push_str(&format!(
                "{:<24} | {:<12} | {:<12} | {:<12} | ratio: {:.4} (reqs: {})\n",
                "3. Range-Request Overhead",
                format!("{:.2} MiB", ro.bytes_transferred as f64 / (1024.0 * 1024.0)),
                "-",
                "-",
                ro.transfer_ratio,
                ro.requests_issued
            ));
        }

        if let Some(r) = &self.resume {
            out.push_str(&format!(
                "{:<24} | {:<12} | {:<12} | {:<12} | re-fetched: {} B ({:.1}%)\n",
                "4. Resume Correctness Cost",
                format!("{:.2} MiB", r.total_bytes_transferred as f64 / (1024.0 * 1024.0)),
                "-",
                "-",
                r.re_fetched_bytes,
                r.re_fetch_ratio * 100.0
            ));
        }

        if let Some(m) = &self.memory {
            let mem_str = match m.peak_rss_mb {
                Some(mb) => format!("{:.2} MiB ({} B)", mb, m.peak_rss_bytes.unwrap_or(0)),
                None => "not measured".to_string(),
            };
            out.push_str(&format!(
                "{:<24} | {:<12} | {:<12} | {:<12} | peak RSS: {}\n",
                "5. Process Memory",
                "-",
                "-",
                "-",
                mem_str
            ));
        }

        out.push_str("========================================================================================\n");
        out
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

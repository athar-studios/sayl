use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Payload received from the frontend to report runtime performance metrics.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct FrontendPerf {
    #[serde(alias = "first_paint_ms")]
    pub first_paint_ms: Option<f64>,
    #[serde(alias = "click_ms")]
    pub click_ms: Option<f64>,
    #[serde(alias = "ipc_total")]
    pub ipc_total: Option<u64>,
    #[serde(alias = "ipc_per_sec")]
    pub ipc_per_sec: Option<f64>,
}

/// JSON line entry written to `perf.log`.
#[derive(Debug, Serialize)]
pub struct PerfEntry<'a> {
    pub event: &'a str,
    pub timestamp_ms: u64,
    pub elapsed_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_paint_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub click_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipc_total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipc_per_sec: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<&'a str>,
}

/// Append-only performance logger enabled via `SAYL_PERF=1`.
pub struct PerfLogger {
    enabled: bool,
    start_instant: Instant,
    file: Option<Mutex<File>>,
}

static GLOBAL_LOGGER: OnceLock<PerfLogger> = OnceLock::new();

impl PerfLogger {
    /// Creates a new `PerfLogger`. When `enabled` is false, file operations are no-ops.
    pub fn new(enabled: bool, path: Option<PathBuf>) -> Self {
        let file = if enabled {
            if let Some(p) = path {
                if let Some(parent) = p.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(p)
                    .ok()
                    .map(Mutex::new)
            } else {
                None
            }
        } else {
            None
        };

        Self {
            enabled,
            start_instant: Instant::now(),
            file,
        }
    }

    /// Returns whether performance logging is currently enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Logs an event with Unix epoch millis and elapsed milliseconds since start.
    pub fn log(&self, event: &str, details: Option<&str>) {
        if !self.enabled {
            return;
        }

        let now_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let elapsed = self.start_instant.elapsed().as_millis() as u64;

        let entry = PerfEntry {
            event,
            timestamp_ms: now_epoch,
            elapsed_ms: elapsed,
            first_paint_ms: None,
            click_ms: None,
            ipc_total: None,
            ipc_per_sec: None,
            details,
        };

        self.write_entry(&entry);
    }

    /// Logs a frontend performance payload.
    pub fn log_frontend(&self, payload: &FrontendPerf) {
        if !self.enabled {
            return;
        }

        let now_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let elapsed = self.start_instant.elapsed().as_millis() as u64;

        let entry = PerfEntry {
            event: "frontend_perf",
            timestamp_ms: now_epoch,
            elapsed_ms: elapsed,
            first_paint_ms: payload.first_paint_ms,
            click_ms: payload.click_ms,
            ipc_total: payload.ipc_total,
            ipc_per_sec: payload.ipc_per_sec,
            details: None,
        };

        self.write_entry(&entry);
    }

    fn write_entry(&self, entry: &PerfEntry<'_>) {
        if let Some(ref lock) = self.file {
            if let Ok(mut f) = lock.lock() {
                if let Ok(json) = serde_json::to_string(entry) {
                    let _ = writeln!(f, "{json}");
                    let _ = f.flush();
                }
            }
        }
    }
}

/// Initializes the global `PerfLogger` with the given data directory.
pub fn init(data_dir: &Path) {
    let enabled = std::env::var("SAYL_PERF")
        .map(|v| v == "1")
        .unwrap_or(false);
    let log_path = if enabled {
        Some(data_dir.join("perf.log"))
    } else {
        None
    };
    let logger = PerfLogger::new(enabled, log_path);
    let _ = GLOBAL_LOGGER.set(logger);
}

/// Returns a reference to the global `PerfLogger`.
pub fn global() -> &'static PerfLogger {
    GLOBAL_LOGGER.get_or_init(|| {
        let enabled = std::env::var("SAYL_PERF")
            .map(|v| v == "1")
            .unwrap_or(false);
        let log_path = if enabled {
            Some(crate::default_data_dir().join("perf.log"))
        } else {
            None
        };
        PerfLogger::new(enabled, log_path)
    })
}

/// Convenience function to log an event to the global logger.
pub fn log_event(event: &str) {
    global().log(event, None);
}

/// Convenience function to log frontend metrics to the global logger.
pub fn log_frontend_perf(payload: &FrontendPerf) {
    global().log_frontend(payload);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_temp_dir(name: &str) -> PathBuf {
        let base = std::env::var("CARGO_TARGET_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join("target")
                    .join("test-tmp")
            });
        let dir = base.join(format!(
            "{name}_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn test_perf_logger_disabled_writes_nothing() {
        let temp_dir = test_temp_dir("perf_disabled");
        let log_file = temp_dir.join("perf.log");

        let logger = PerfLogger::new(false, Some(log_file.clone()));
        assert!(!logger.is_enabled());

        logger.log("process_start", None);
        logger.log("window_shown", None);
        logger.log_frontend(&FrontendPerf {
            first_paint_ms: Some(150.0),
            ..Default::default()
        });

        assert!(
            !log_file.exists() || std::fs::read_to_string(&log_file).unwrap().is_empty(),
            "Disabled logger must not write to file"
        );
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_perf_logger_enabled_writes_events() {
        let temp_dir = test_temp_dir("perf_enabled");
        let log_file = temp_dir.join("perf.log");

        let logger = PerfLogger::new(true, Some(log_file.clone()));
        assert!(logger.is_enabled());

        logger.log("process_start", None);
        logger.log("engine_init", None);
        logger.log("setup_done", None);
        logger.log("window_shown", None);
        logger.log_frontend(&FrontendPerf {
            first_paint_ms: Some(420.5),
            click_ms: Some(45.0),
            ipc_total: Some(10),
            ipc_per_sec: Some(2.5),
        });
        logger.log("exit", None);

        assert!(log_file.exists());
        let content = std::fs::read_to_string(&log_file).expect("readable log");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 6);

        let parsed: Vec<serde_json::Value> = lines
            .into_iter()
            .map(|l| serde_json::from_str(l).expect("valid JSON line"))
            .collect();

        assert_eq!(parsed[0]["event"], "process_start");
        assert!(parsed[0]["timestamp_ms"].as_u64().unwrap() > 0);
        assert_eq!(parsed[1]["event"], "engine_init");
        assert_eq!(parsed[2]["event"], "setup_done");
        assert_eq!(parsed[3]["event"], "window_shown");

        assert_eq!(parsed[4]["event"], "frontend_perf");
        assert_eq!(parsed[4]["first_paint_ms"], 420.5);
        assert_eq!(parsed[4]["click_ms"], 45.0);
        assert_eq!(parsed[4]["ipc_total"], 10);
        assert_eq!(parsed[4]["ipc_per_sec"], 2.5);

        assert_eq!(parsed[5]["event"], "exit");
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_frontend_perf_serde_compatibility() {
        let json_camel = r#"{
            "firstPaintMs": 350.5,
            "clickMs": 25.0,
            "ipcTotal": 100,
            "ipcPerSec": 4.2
        }"#;
        let parsed: FrontendPerf = serde_json::from_str(json_camel).expect("camelCase parsing");
        assert_eq!(parsed.first_paint_ms, Some(350.5));
        assert_eq!(parsed.click_ms, Some(25.0));
        assert_eq!(parsed.ipc_total, Some(100));
        assert_eq!(parsed.ipc_per_sec, Some(4.2));

        let json_snake = r#"{
            "first_paint_ms": 120.0,
            "click_ms": 10.0
        }"#;
        let parsed_snake: FrontendPerf =
            serde_json::from_str(json_snake).expect("snake_case alias parsing");
        assert_eq!(parsed_snake.first_paint_ms, Some(120.0));
        assert_eq!(parsed_snake.click_ms, Some(10.0));
        assert_eq!(parsed_snake.ipc_total, None);
    }
}

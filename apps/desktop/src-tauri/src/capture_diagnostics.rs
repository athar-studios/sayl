//! Extension capture diagnostics for Settings (staged manifest, browsers, bridge ping, log tail).

use std::fs;
use std::path::PathBuf;

use sayl_bridge::install::chromium_manifest_path;
use sayl_bridge::ipc::capture_log_path;
use serde::{Deserialize, Serialize};

use crate::browsers;
use crate::default_data_dir;
use sayl_bridge::install::WindowsRegistry;

use crate::native_host::{resolve_bridge_exe, run_bridge_ping};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserDiagnosticRow {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub host_key_present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CaptureLogTailSummary {
    pub last_capture_event: Option<String>,
    pub last_capture_time: Option<String>,
    pub last_error_event: Option<String>,
    pub last_error_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtensionDiagnostics {
    pub extension_version: Option<String>,
    pub extension_path: String,
    pub browsers: Vec<BrowserDiagnosticRow>,
    pub bridge_ping_reached: bool,
    pub bridge_ping_message: String,
    pub last_capture_event: Option<String>,
    pub last_capture_time: Option<String>,
    pub last_error_event: Option<String>,
    pub last_error_time: Option<String>,
}

/// Parses the tail of `capture.log` JSON lines for last capture and error events.
pub fn parse_capture_log_tail(content: &str, max_lines: usize) -> CaptureLogTailSummary {
    let mut lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.len() > max_lines {
        lines = lines.split_off(lines.len().saturating_sub(max_lines));
    }

    let mut summary = CaptureLogTailSummary::default();

    for line in lines.iter().rev() {
        let Ok(val) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let event = val
            .get("event")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let timestamp = val
            .get("timestamp")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let level = val
            .get("level")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        if summary.last_error_event.is_none() && level == "error" && !event.is_empty() {
            summary.last_error_event = Some(event.clone());
            summary.last_error_time = timestamp.clone();
        }

        if summary.last_capture_event.is_none()
            && !event.is_empty()
            && (event.to_ascii_lowercase().contains("capture")
                || event.to_ascii_lowercase().contains("download"))
        {
            summary.last_capture_event = Some(event);
            summary.last_capture_time = timestamp;
        }

        if summary.last_capture_event.is_some() && summary.last_error_event.is_some() {
            break;
        }
    }

    summary
}

fn read_staged_extension_version() -> (Option<String>, String) {
    let path = chromium_manifest_path();
    let path_display = path.to_string_lossy().to_string();
    if !path.is_file() {
        return (None, path_display);
    }
    let Ok(text) = fs::read_to_string(&path) else {
        return (None, path_display);
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) else {
        return (None, path_display);
    };
    let version = val
        .get("version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    (version, path_display)
}

fn read_log_tail(data_dir: &PathBuf) -> CaptureLogTailSummary {
    let log_path = capture_log_path(Some(data_dir));
    let Ok(content) = fs::read_to_string(&log_path) else {
        return CaptureLogTailSummary::default();
    };
    parse_capture_log_tail(&content, 200)
}

pub async fn collect_extension_diagnostics() -> ExtensionDiagnostics {
    let data_dir = default_data_dir();
    let (extension_version, extension_path) = read_staged_extension_version();

    let reg = WindowsRegistry;
    let probe = browsers::SystemBrowserProbe;
    let browser_statuses =
        browsers::collect_browser_statuses(&probe, &reg, false);

    let browsers: Vec<BrowserDiagnosticRow> = browser_statuses
        .into_iter()
        .map(|b| BrowserDiagnosticRow {
            id: b.id,
            name: b.name,
            installed: b.installed,
            host_key_present: b.host_registered,
        })
        .collect();

    let ping = match resolve_bridge_exe() {
        Ok(exe) => run_bridge_ping(&exe).await,
        Err(e) => crate::native_host::NativeHostPing {
            ok: false,
            roundtrip_ms: 0,
            message: e,
        },
    };

    let log_tail = read_log_tail(&data_dir);

    ExtensionDiagnostics {
        extension_version,
        extension_path,
        browsers,
        bridge_ping_reached: ping.ok,
        bridge_ping_message: ping.message,
        last_capture_event: log_tail.last_capture_event,
        last_capture_time: log_tail.last_capture_time,
        last_error_event: log_tail.last_error_event,
        last_error_time: log_tail.last_error_time,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_capture_log_tail_extracts_last_capture_and_error() {
        let content = r#"{"timestamp":"2026-01-01T08:00:00Z","level":"info","event":"download_created","detail":{}}
{"timestamp":"2026-01-01T08:01:00Z","level":"info","event":"capture_offer","detail":{}}
{"timestamp":"2026-01-01T08:02:00Z","level":"error","event":"forward_failed","detail":{}}"#;

        let summary = parse_capture_log_tail(content, 50);
        assert_eq!(
            summary.last_capture_event.as_deref(),
            Some("capture_offer")
        );
        assert_eq!(
            summary.last_capture_time.as_deref(),
            Some("2026-01-01T08:01:00Z")
        );
        assert_eq!(
            summary.last_error_event.as_deref(),
            Some("forward_failed")
        );
        assert_eq!(
            summary.last_error_time.as_deref(),
            Some("2026-01-01T08:02:00Z")
        );
    }
}

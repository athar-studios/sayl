//! Capture log appending, sanitization, and 5 MiB rotation.

use serde_json::{json, Value};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::ipc::capture_log_path;

const MAX_LOG_SIZE_BYTES: u64 = 5 * 1024 * 1024;

/// Sanitizes a URL string by keeping only scheme, host, and path (stripping query and fragment).
pub fn sanitize_url(raw: &str) -> String {
    match url::Url::parse(raw) {
        Ok(u) => {
            let host = u.host_str().unwrap_or("");
            let path = u.path();
            format!("{}://{}{}", u.scheme(), host, path)
        }
        Err(_) => "<invalid-url>".to_string(),
    }
}

/// Recursively sanitizes JSON values to remove cookie values, tokens, and query strings.
pub fn sanitize_detail(val: &Value) -> Value {
    match val {
        Value::Object(map) => {
            let mut sanitized_map = serde_json::Map::new();
            for (k, v) in map {
                let k_lower = k.to_ascii_lowercase();
                if k_lower.contains("cookie") {
                    if let Value::Array(arr) = v {
                        sanitized_map.insert(format!("{}_count", k), json!(arr.len()));
                    } else {
                        sanitized_map.insert(k.clone(), json!("[REDACTED]"));
                    }
                } else if k_lower.contains("token")
                    || k_lower.contains("auth")
                    || k_lower.contains("secret")
                    || k_lower.contains("password")
                {
                    sanitized_map.insert(k.clone(), json!("[REDACTED]"));
                } else if k_lower.contains("url") {
                    if let Value::String(s) = v {
                        sanitized_map.insert(k.clone(), json!(sanitize_url(s)));
                    } else {
                        sanitized_map.insert(k.clone(), sanitize_detail(v));
                    }
                } else {
                    sanitized_map.insert(k.clone(), sanitize_detail(v));
                }
            }
            Value::Object(sanitized_map)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(sanitize_detail).collect()),
        Value::String(s) => {
            if s.contains("token=") || s.contains("secret=") || s.contains("cookie=") {
                json!("[REDACTED]")
            } else {
                Value::String(s.clone())
            }
        }
        other => other.clone(),
    }
}

/// Appends a structured JSON line to `capture.log`, rotating to `capture.log.1` at 5 MiB.
pub fn append_log(
    data_dir: Option<&Path>,
    level: &str,
    event: &str,
    detail: Value,
) -> Result<(), String> {
    let log_path: PathBuf = capture_log_path(data_dir);
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // Check size for rotation
    if let Ok(meta) = std::fs::metadata(&log_path) {
        if meta.len() >= MAX_LOG_SIZE_BYTES {
            let rotated = log_path.with_file_name("capture.log.1");
            let _ = std::fs::remove_file(&rotated);
            let _ = std::fs::rename(&log_path, &rotated);
        }
    }

    let timestamp = chrono::Utc::now().to_rfc3339();
    let sanitized_detail = sanitize_detail(&detail);

    let log_entry = json!({
        "timestamp": timestamp,
        "level": level,
        "event": event,
        "detail": sanitized_detail,
    });

    let mut line = serde_json::to_string(&log_entry)
        .map_err(|e| format!("failed to serialize log entry: {e}"))?;
    line.push('\n');

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("failed to open log file {}: {e}", log_path.display()))?;

    file.write_all(line.as_bytes())
        .map_err(|e| format!("failed to write to log file {}: {e}", log_path.display()))?;
    file.flush()
        .map_err(|e| format!("failed to flush log file {}: {e}", log_path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_url_removes_query_and_fragment() {
        let cleaned = sanitize_url("https://example.com/download.zip?token=secret123#frag");
        assert_eq!(cleaned, "https://example.com/download.zip");
    }

    #[test]
    fn test_sanitize_detail_redacts_cookies_and_tokens() {
        let raw = json!({
            "cookies": [{"name": "foo", "value": "bar"}],
            "cookie": "my_secret_cookie_string",
            "token": "secret_token_123",
            "download_url": "https://example.com/a.zip?key=private",
            "safe_field": 42
        });
        let cleaned = sanitize_detail(&raw);
        let s = cleaned.to_string();

        assert!(!s.contains("bar"));
        assert!(!s.contains("my_secret_cookie_string"));
        assert!(!s.contains("secret_token_123"));
        assert!(!s.contains("key=private"));
        assert_eq!(cleaned["cookies_count"], 1);
        assert_eq!(cleaned["cookie"], "[REDACTED]");
        assert_eq!(cleaned["token"], "[REDACTED]");
        assert_eq!(cleaned["download_url"], "https://example.com/a.zip");
        assert_eq!(cleaned["safe_field"], 42);
    }

    #[test]
    fn test_append_log_creates_file_and_writes_json_line() {
        let temp = tempfile::tempdir().unwrap();
        append_log(
            Some(temp.path()),
            "info",
            "test_event",
            json!({"test_key": "test_val"}),
        )
        .expect("write log");

        let log_file = temp.path().join("logs").join("capture.log");
        assert!(log_file.is_file());
        let content = std::fs::read_to_string(&log_file).unwrap();
        assert!(content.contains("\"event\":\"test_event\""));
        assert!(content.contains("\"level\":\"info\""));
    }

    #[test]
    fn test_log_rotation_at_limit() {
        let temp = tempfile::tempdir().unwrap();
        let log_dir = temp.path().join("logs");
        std::fs::create_dir_all(&log_dir).unwrap();
        let log_file = log_dir.join("capture.log");

        // Create a 5 MiB dummy file
        let dummy = vec![b'a'; MAX_LOG_SIZE_BYTES as usize];
        std::fs::write(&log_file, dummy).unwrap();

        append_log(
            Some(temp.path()),
            "info",
            "new_event_after_rotation",
            json!({}),
        )
        .expect("write log");

        let rotated_file = log_dir.join("capture.log.1");
        assert!(rotated_file.is_file(), "rotated log must exist");
        assert_eq!(
            std::fs::metadata(&rotated_file).unwrap().len(),
            MAX_LOG_SIZE_BYTES
        );

        let new_content = std::fs::read_to_string(&log_file).unwrap();
        assert!(new_content.contains("new_event_after_rotation"));
    }
}

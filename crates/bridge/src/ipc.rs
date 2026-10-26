//! Loopback IPC configuration and discovery for SAYL.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// IPC file payload written by SAYL desktop app at `%APPDATA%\SAYL\capture-ipc.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureIpcInfo {
    /// Loopback TCP port bound by the app.
    pub port: u16,
    /// 32-byte secret authorization token encoded as 64 hex characters.
    pub token: String,
    /// Process ID of the running desktop application.
    pub pid: u32,
    /// Protocol version spoken by the IPC endpoint (currently 1).
    pub protocol: u32,
}

/// Resolves the default data directory for SAYL database, config, and IPC file.
pub fn default_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("SAYL_DATA_DIR") {
        return PathBuf::from(dir);
    }
    if let Some(app_data) = std::env::var_os("APPDATA") {
        let candidate = PathBuf::from(app_data).join("SAYL");
        if std::fs::create_dir_all(&candidate).is_ok() {
            return candidate;
        }
    }
    if let Some(user_dirs) = std::env::var_os("USERPROFILE") {
        let candidate = PathBuf::from(user_dirs)
            .join("AppData")
            .join("Roaming")
            .join("SAYL");
        if std::fs::create_dir_all(&candidate).is_ok() {
            return candidate;
        }
    }
    std::env::temp_dir().join("SAYL")
}

/// Resolves the absolute path to `capture-ipc.json`.
pub fn capture_ipc_path(data_dir: Option<&Path>) -> PathBuf {
    match data_dir {
        Some(dir) => dir.join("capture-ipc.json"),
        None => default_data_dir().join("capture-ipc.json"),
    }
}

/// Resolves the absolute path to `capture.log`.
pub fn capture_log_path(data_dir: Option<&Path>) -> PathBuf {
    match data_dir {
        Some(dir) => dir.join("logs").join("capture.log"),
        None => default_data_dir().join("logs").join("capture.log"),
    }
}

/// Reads and validates the loopback IPC file.
pub fn read_ipc_info(path: &Path) -> Result<CaptureIpcInfo, String> {
    if !path.is_file() {
        return Err(format!("IPC file does not exist at {}", path.display()));
    }

    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read IPC file {}: {e}", path.display()))?;

    let info: CaptureIpcInfo = serde_json::from_str(&contents)
        .map_err(|e| format!("failed to parse IPC json from {}: {e}", path.display()))?;

    if info.port == 0 {
        return Err("IPC port is invalid (0)".to_string());
    }

    if info.token.len() != 64 || !info.token.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("IPC token must be a 64-character hex string".to_string());
    }

    if info.protocol != 1 {
        return Err(format!(
            "unsupported IPC protocol version: {}",
            info.protocol
        ));
    }

    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_ipc_file_parsing() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("capture-ipc.json");
        let valid_token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let content = format!(
            r#"{{"port":54321,"token":"{}","pid":1234,"protocol":1}}"#,
            valid_token
        );
        std::fs::write(&path, content).unwrap();

        let info = read_ipc_info(&path).expect("valid ipc info");
        assert_eq!(info.port, 54321);
        assert_eq!(info.token, valid_token);
        assert_eq!(info.pid, 1234);
        assert_eq!(info.protocol, 1);
    }

    #[test]
    fn test_invalid_token_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("capture-ipc.json");
        let content = r#"{"port":54321,"token":"short","pid":1234,"protocol":1}"#;
        std::fs::write(&path, content).unwrap();

        assert!(read_ipc_info(&path).is_err());
    }
}

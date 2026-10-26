//! Loopback HTTP IPC server for desktop app integration.
//!
//! Exposes `GET /health` and `POST /capture` on `127.0.0.1:<port>`.
//! Writes port, secret token, PID, and protocol version to `capture-ipc.json`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use sayl_app::Engine;
use sayl_bridge::ipc::{capture_ipc_path, CaptureIpcInfo};
use sayl_bridge::log::{append_log, sanitize_url};
use sayl_bridge::BrowserCookie;
use serde::Deserialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

pub const MAX_BODY_BYTES: usize = 256 * 1024; // 256 KiB
pub const MAX_HEADER_BYTES: usize = 32 * 1024; // 32 KiB

/// Payload for `POST /capture` request.
#[derive(Debug, Clone, Deserialize)]
pub struct CapturePayload {
    pub url: String,
    #[serde(default)]
    pub final_url: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub referrer: Option<String>,
    #[serde(default)]
    pub cookies: Vec<BrowserCookie>,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub mime: Option<String>,
}

/// Handle to the running capture server allowing graceful shutdown and cleanup.
pub struct CaptureServerHandle {
    pub port: u16,
    pub token: String,
    pub ipc_path: PathBuf,
    shutdown_tx: Option<oneshot::Sender<()>>,
    server_task: Option<tokio::task::JoinHandle<()>>,
}

impl CaptureServerHandle {
    /// Signals the capture server to stop and removes `capture-ipc.json`.
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(task) = self.server_task.take() {
            task.abort();
        }
        let _ = std::fs::remove_file(&self.ipc_path);
    }
}

impl Drop for CaptureServerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Generates a 32-byte cryptographically secure random token encoded as 64 hex chars.
pub fn generate_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| format!("getrandom failed: {e}"))?;
    let mut hex = String::with_capacity(64);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(hex, "{:02x}", b);
    }
    Ok(hex)
}

/// Atomically writes `capture-ipc.json` (temp file + rename).
pub fn write_ipc_file_atomic(path: &Path, info: &CaptureIpcInfo) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json_bytes = serde_json::to_vec_pretty(info)
        .map_err(|e| format!("failed to serialize IPC info: {e}"))?;

    let temp_name = format!("capture-ipc.{}.tmp", std::process::id());
    let temp_path = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(temp_name);

    std::fs::write(&temp_path, &json_bytes)
        .map_err(|e| format!("failed to write temp IPC file: {e}"))?;

    if let Err(e) = std::fs::rename(&temp_path, path) {
        let _ = std::fs::remove_file(path);
        std::fs::rename(&temp_path, path).map_err(|e2| {
            let _ = std::fs::remove_file(&temp_path);
            format!("failed to atomically rename IPC file (first error: {e}): {e2}")
        })?;
    }
    Ok(())
}

async fn write_response(
    stream: &mut TcpStream,
    status_code: u16,
    reason: &str,
    body: &str,
) -> std::io::Result<()> {
    let resp = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status_code,
        reason,
        body.len(),
        body
    );
    stream.write_all(resp.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}

#[derive(Clone, serde::Serialize)]
pub struct CaptureOfferPayload {
    pub task_id: String,
    pub filename: String,
    pub size: Option<u64>,
    pub url: String,
}

async fn handle_connection(
    mut stream: TcpStream,
    expected_token: Arc<String>,
    engine: Engine,
    app_handle: Option<AppHandle>,
    data_dir: PathBuf,
) -> std::io::Result<()> {
    let mut buffer = Vec::with_capacity(16 * 1024);
    let mut chunk = [0u8; 4096];

    // Read until headers end (\r\n\r\n or \n\n)
    let header_end_pos;
    loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Ok(());
        }
        buffer.extend_from_slice(&chunk[..n]);

        if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end_pos = pos + 4;
            break;
        }
        if let Some(pos) = buffer.windows(2).position(|w| w == b"\n\n") {
            header_end_pos = pos + 2;
            break;
        }
        if buffer.len() > MAX_HEADER_BYTES {
            let err_body = json!({"status": "error", "message": "headers too large"}).to_string();
            return write_response(
                &mut stream,
                431,
                "Request Header Fields Too Large",
                &err_body,
            )
            .await;
        }
    }

    let header_bytes = &buffer[..header_end_pos];
    let header_str = String::from_utf8_lossy(header_bytes);
    let mut lines = header_str.lines();

    let request_line = match lines.next() {
        Some(l) => l,
        None => return Ok(()),
    };

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_uppercase();
    let path = parts.next().unwrap_or("").to_string();

    let mut auth_header: Option<String> = None;
    let mut content_length: Option<usize> = None;

    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            let k_trimmed = k.trim().to_ascii_lowercase();
            let v_trimmed = v.trim();
            if k_trimmed == "authorization" {
                auth_header = Some(v_trimmed.to_string());
            } else if k_trimmed == "content-length" {
                content_length = v_trimmed.parse::<usize>().ok();
            }
        }
    }

    // Route: GET /health (no auth)
    if path == "/health" {
        if method != "GET" {
            let err = json!({"status": "error", "message": "method not allowed"}).to_string();
            return write_response(&mut stream, 405, "Method Not Allowed", &err).await;
        }
        let body = json!({
            "status": "ok",
            "app_version": env!("CARGO_PKG_VERSION"),
            "protocol": 1,
        })
        .to_string();
        return write_response(&mut stream, 200, "OK", &body).await;
    }

    // Route: POST /capture (requires token auth, 256 KiB body cap)
    if path == "/capture" {
        if method != "POST" {
            let err = json!({"status": "error", "message": "method not allowed"}).to_string();
            return write_response(&mut stream, 405, "Method Not Allowed", &err).await;
        }

        // Validate Authorization header
        let expected_auth = format!("Bearer {}", expected_token);
        match auth_header {
            Some(ref h) if h == &expected_auth => {}
            _ => {
                let err = json!({"status": "error", "message": "unauthorized"}).to_string();
                return write_response(&mut stream, 401, "Unauthorized", &err).await;
            }
        }

        // Validate body cap
        let cl = match content_length {
            Some(len) => len,
            None => {
                let err =
                    json!({"status": "error", "message": "missing content-length"}).to_string();
                return write_response(&mut stream, 400, "Bad Request", &err).await;
            }
        };

        if cl > MAX_BODY_BYTES {
            let err = json!({"status": "error", "message": "payload too large"}).to_string();
            let _ = write_response(&mut stream, 413, "Payload Too Large", &err).await;
            // Drain unread bytes (capped to 1 MiB) to avoid Windows TCP RST
            let mut remaining = cl.saturating_sub(buffer.len().saturating_sub(header_end_pos));
            while remaining > 0 {
                let to_read = remaining.min(chunk.len());
                match tokio::time::timeout(
                    Duration::from_millis(500),
                    stream.read(&mut chunk[..to_read]),
                )
                .await
                {
                    Ok(Ok(0)) | Ok(Err(_)) | Err(_) => break,
                    Ok(Ok(n)) => remaining = remaining.saturating_sub(n),
                }
            }
            return Ok(());
        }

        // Read remaining body bytes if needed
        let mut body = buffer[header_end_pos..].to_vec();
        while body.len() < cl {
            let n = stream.read(&mut chunk).await?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..n]);
        }

        if body.len() < cl {
            let err = json!({"status": "error", "message": "incomplete body"}).to_string();
            return write_response(&mut stream, 400, "Bad Request", &err).await;
        }

        let body_slice = &body[..cl];
        let payload: CapturePayload = match serde_json::from_slice(body_slice) {
            Ok(p) => p,
            Err(e) => {
                let err = json!({"status": "error", "message": format!("invalid payload: {e}")})
                    .to_string();
                return write_response(&mut stream, 400, "Bad Request", &err).await;
            }
        };

        // Determine effective target URL
        let target_url = payload
            .final_url
            .as_ref()
            .filter(|u| !u.trim().is_empty())
            .unwrap_or(&payload.url);

        let parsed_url = match url::Url::parse(target_url) {
            Ok(u) => u,
            Err(e) => {
                let err =
                    json!({"status": "error", "message": format!("invalid url: {e}")}).to_string();
                return write_response(&mut stream, 400, "Bad Request", &err).await;
            }
        };

        if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
            let err = json!({"status": "error", "message": "url scheme must be http or https"})
                .to_string();
            return write_response(&mut stream, 400, "Bad Request", &err).await;
        }

        // Validate size sanity
        if let Some(s) = payload.size {
            if s > 100_000_000_000_000 {
                let err = json!({"status": "error", "message": "invalid size"}).to_string();
                return write_response(&mut stream, 400, "Bad Request", &err).await;
            }
        }
        let size_or_none = payload.size.filter(|&s| s > 0);

        // Sanitize filename
        let sanitized_filename = payload
            .filename
            .as_deref()
            .filter(|f| !f.trim().is_empty())
            .map(sayl_http::filename::sanitize_filename);

        // Build request headers: Cookie, User-Agent, Referer
        let mut custom_headers = Vec::new();
        if !payload.cookies.is_empty() {
            let cookie_str = payload
                .cookies
                .iter()
                .map(|c| format!("{}={}", c.name, c.value))
                .collect::<Vec<_>>()
                .join("; ");
            custom_headers.push(("Cookie".to_string(), cookie_str));
        }
        if let Some(ref ua) = payload.user_agent {
            if !ua.trim().is_empty() {
                custom_headers.push(("User-Agent".to_string(), ua.clone()));
            }
        }
        if let Some(ref referer) = payload.referrer {
            if !referer.trim().is_empty() {
                custom_headers.push(("Referer".to_string(), referer.clone()));
            }
        }

        // Queue task with metadata; download bytes start only after UI confirm.
        match engine
            .add_with_metadata(
                target_url,
                sanitized_filename.clone(),
                size_or_none,
                payload.referrer.clone(),
                Some(custom_headers),
            )
            .await
        {
            Ok(task_id) => {
                let filename_for_ui = sanitized_filename.unwrap_or_else(|| "download".to_string());
                let offer = CaptureOfferPayload {
                    task_id: task_id.to_string(),
                    filename: filename_for_ui.clone(),
                    size: size_or_none,
                    url: target_url.to_string(),
                };

                let _ = append_log(
                    Some(&data_dir),
                    "info",
                    "capture_offer",
                    json!({
                        "task_id": task_id.to_string(),
                        "filename": filename_for_ui,
                        "size": size_or_none,
                        "url": sanitize_url(target_url),
                    }),
                );

                if let Some(handle) = app_handle.as_ref() {
                    if let Some(window) = handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                    let _ = handle.emit("sayl://capture-offer", &offer);
                }

                let resp_body = json!({
                    "status": "accepted",
                    "task_id": task_id.to_string(),
                })
                .to_string();
                return write_response(&mut stream, 200, "OK", &resp_body).await;
            }
            Err(e) => {
                let err = json!({"status": "error", "message": e.to_string()}).to_string();
                return write_response(&mut stream, 400, "Bad Request", &err).await;
            }
        }
    }

    // Default 404
    let err = json!({"status": "error", "message": "not found"}).to_string();
    write_response(&mut stream, 404, "Not Found", &err).await
}

/// Starts the capture loopback server on `127.0.0.1:0`.
///
/// Injects `engine`, `data_dir`, and optional `custom_token`.
/// Atomically writes `capture-ipc.json`.
pub async fn start_capture_server(
    engine: Engine,
    data_dir: PathBuf,
    custom_token: Option<String>,
    app_handle: Option<AppHandle>,
) -> Result<CaptureServerHandle, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("failed to bind loopback TCP listener on port 0: {e}"))?;

    let local_addr = listener
        .local_addr()
        .map_err(|e| format!("failed to get listener local_addr: {e}"))?;
    let port = local_addr.port();

    let token = match custom_token {
        Some(t) => t,
        None => generate_token()?,
    };

    let ipc_path = capture_ipc_path(Some(&data_dir));
    let ipc_info = CaptureIpcInfo {
        port,
        token: token.clone(),
        pid: std::process::id(),
        protocol: 1,
    };

    write_ipc_file_atomic(&ipc_path, &ipc_info)?;

    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
    let token_arc = Arc::new(token.clone());

    let data_dir_for_conn = data_dir.clone();
    let server_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                accept_res = listener.accept() => {
                    match accept_res {
                        Ok((stream, _)) => {
                            let token_clone = Arc::clone(&token_arc);
                            let engine_clone = engine.clone();
                            let app_handle_clone = app_handle.clone();
                            let data_dir_clone = data_dir_for_conn.clone();
                            tokio::spawn(async move {
                                let _ = tokio::time::timeout(
                                    Duration::from_secs(15),
                                    handle_connection(
                                        stream,
                                        token_clone,
                                        engine_clone,
                                        app_handle_clone,
                                        data_dir_clone,
                                    ),
                                ).await;
                            });
                        }
                        Err(e) => {
                            eprintln!("Capture IPC listener accept error: {e}");
                        }
                    }
                }
                _ = &mut shutdown_rx => {
                    break;
                }
            }
        }
    });

    Ok(CaptureServerHandle {
        port,
        token,
        ipc_path,
        shutdown_tx: Some(shutdown_tx),
        server_task: Some(server_task),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sayl_app::EngineConfig;
    use sayl_core::state::DownloadState;

    static TEST_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

    fn make_test_engine_and_dirs() -> (Engine, PathBuf, PathBuf, PathBuf) {
        let counter = TEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let unique = format!(
            "desktop-test-{}-{}-{}",
            std::process::id(),
            counter,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let base = std::env::temp_dir().join(unique);
        let data_dir = base.join("data");
        let dl_dir = base.join("dl");
        let store_dir = base.join("store");

        std::fs::create_dir_all(&data_dir).unwrap();
        std::fs::create_dir_all(&dl_dir).unwrap();
        std::fs::create_dir_all(&store_dir).unwrap();

        let engine = Engine::new(&dl_dir, &store_dir, EngineConfig::default()).unwrap();
        (engine, base, data_dir, dl_dir)
    }

    async fn send_raw_request(port: u16, raw: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .unwrap();
        stream.write_all(raw.as_bytes()).await.unwrap();
        stream.flush().await.unwrap();
        let _ = stream.shutdown().await;
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();
        let resp_str = String::from_utf8_lossy(&buf).to_string();
        let status_code = resp_str
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u16>().ok())
            .unwrap_or(0);
        let body = resp_str.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
        (status_code, body)
    }

    #[tokio::test]
    async fn test_capture_server_health_ok() {
        let (engine, base, data_dir, _) = make_test_engine_and_dirs();
        let handle = start_capture_server(engine, data_dir.clone(), None, None)
            .await
            .expect("start capture server");

        let raw_req = "GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
        let (status, body) = send_raw_request(handle.port, raw_req).await;

        assert_eq!(status, 200);
        let val: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(val["status"], "ok");
        assert_eq!(val["app_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(val["protocol"], 1);

        drop(handle);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn test_capture_server_post_capture_with_metadata_accepted_and_task_exists() {
        let (engine, base, data_dir, _) = make_test_engine_and_dirs();
        let handle = start_capture_server(engine.clone(), data_dir.clone(), None, None)
            .await
            .expect("start capture server");

        let payload = json!({
            "type": "Capture",
            "version": 1,
            "url": "https://example.com/archive.zip",
            "final_url": "https://cdn.example.com/archive.zip",
            "filename": "archive.zip",
            "referrer": "https://referrer.example.com",
            "cookies": [{"name": "auth", "value": "test_token"}],
            "user_agent": "TestBrowser/1.0",
            "size": 65536,
            "mime": "application/zip"
        })
        .to_string();

        let raw_req = format!(
            "POST /capture HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            handle.token,
            payload.len(),
            payload
        );

        let (status, body) = send_raw_request(handle.port, &raw_req).await;
        assert_eq!(status, 200);

        let val: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(val["status"], "accepted");
        let task_id_str = val["task_id"].as_str().expect("task_id present");

        // Verify task exists in the real engine
        let task_id: sayl_core::id::TaskId = task_id_str.parse().unwrap();
        let task_record = engine.get_task(task_id).unwrap();
        assert!(task_record.is_some(), "task must exist in engine store");
        let rec = task_record.unwrap();
        assert_eq!(rec.filename, "archive.zip");
        assert_eq!(rec.total_size, Some(65536));
        assert_eq!(rec.state, DownloadState::Queued);

        drop(handle);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn test_capture_server_github_signed_final_url_uses_final_url_for_task() {
        let (engine, base, data_dir, _) = make_test_engine_and_dirs();
        let handle = start_capture_server(engine.clone(), data_dir.clone(), None, None)
            .await
            .expect("start capture server");

        let release_url =
            "https://github.com/athar-studios/sayl/releases/download/v0.1.0/SAYL-Setup-x64.exe";
        let signed_final = "https://release-assets.githubusercontent.com/github-production-release-asset-2e65bee/123/abc?sp=r&sig=token";

        let payload = json!({
            "url": release_url,
            "final_url": signed_final,
            "filename": "SAYL-Setup-x64.exe",
            "referrer": "https://github.com/athar-studios/sayl/releases",
            "size": 6522100,
            "mime": "application/octet-stream"
        })
        .to_string();

        let raw_req = format!(
            "POST /capture HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            handle.token,
            payload.len(),
            payload
        );

        let (status, body) = send_raw_request(handle.port, &raw_req).await;
        assert_eq!(status, 200);

        let val: serde_json::Value = serde_json::from_str(&body).unwrap();
        let task_id_str = val["task_id"].as_str().expect("task_id present");
        let task_id: sayl_core::id::TaskId = task_id_str.parse().unwrap();
        let rec = engine.get_task(task_id).unwrap().expect("task exists");
        assert_eq!(rec.filename, "SAYL-Setup-x64.exe");
        assert_eq!(rec.url.as_str(), signed_final);
        assert_eq!(rec.state, DownloadState::Queued);

        drop(handle);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn test_capture_server_bad_token_returns_401() {
        let (engine, base, data_dir, _) = make_test_engine_and_dirs();
        let handle = start_capture_server(engine, data_dir.clone(), None, None)
            .await
            .expect("start capture server");

        let payload = json!({
            "url": "https://example.com/file.bin",
            "filename": "file.bin",
            "size": 1024
        })
        .to_string();

        // Request with bad token
        let raw_req = format!(
            "POST /capture HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer invalid_secret_token_12345\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            payload.len(),
            payload
        );
        let (status, _) = send_raw_request(handle.port, &raw_req).await;
        assert_eq!(status, 401);

        // Request with missing token
        let raw_req_missing = format!(
            "POST /capture HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            payload.len(),
            payload
        );
        let (status_missing, _) = send_raw_request(handle.port, &raw_req_missing).await;
        assert_eq!(status_missing, 401);

        drop(handle);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn test_capture_server_oversized_body_returns_413() {
        let (engine, base, data_dir, _) = make_test_engine_and_dirs();
        let handle = start_capture_server(engine, data_dir.clone(), None, None)
            .await
            .expect("start capture server");

        // Construct oversized body > 256 KiB
        let padding = "x".repeat(MAX_BODY_BYTES + 1024);
        let payload = json!({
            "url": "https://example.com/file.bin",
            "filename": padding,
            "size": 1024
        })
        .to_string();

        let raw_req = format!(
            "POST /capture HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            handle.token,
            payload.len(),
            payload
        );

        let (status, _) = send_raw_request(handle.port, &raw_req).await;
        assert_eq!(status, 413);

        drop(handle);
        let _ = std::fs::remove_dir_all(&base);
    }
}

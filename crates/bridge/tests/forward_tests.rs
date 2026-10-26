//! Integration tests for ForwardingDispatcher.

use std::path::PathBuf;
use std::time::Duration;

use sayl_bridge::host::Dispatcher;
use sayl_bridge::ipc::CaptureIpcInfo;
use sayl_bridge::message::{BrowserCookie, Request, Response};
use sayl_bridge::{build_app_launch_command, ForwardingDispatcher};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn make_temp_dir() -> PathBuf {
    let unique = format!(
        "bridge-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let path = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn test_build_app_launch_command_stdio_and_flags() {
    let dummy_path = std::path::Path::new("dummy/sayl.exe");
    let cmd = build_app_launch_command(dummy_path);
    assert!(
        cmd.get_stdin().is_some(),
        "stdin must be redirected to null"
    );
    assert!(
        cmd.get_stdout().is_some(),
        "stdout must be redirected to null"
    );
    assert!(
        cmd.get_stderr().is_some(),
        "stderr must be redirected to null"
    );
}

#[test]
fn test_forwarding_dispatcher_against_fake_app_listener() {
    let temp = make_temp_dir();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    // Start fake app listener on loopback
    let (port, server_handle) = runtime.block_on(async move {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let handle = tokio::spawn(async move {
            // First request should be /health
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let n = socket.read(&mut buf).await.unwrap();
            let req_str = String::from_utf8_lossy(&buf[..n]);
            assert!(req_str.contains("GET /health"));

            let health_body = r#"{"status":"ok","app_version":"0.1.0","protocol":1}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                health_body.len(),
                health_body
            );
            socket.write_all(resp.as_bytes()).await.unwrap();
            socket.flush().await.unwrap();
            drop(socket);

            // Second request should be POST /capture
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 8192];
            let n = socket.read(&mut buf).await.unwrap();
            let req_str = String::from_utf8_lossy(&buf[..n]);
            assert!(req_str.contains("POST /capture"));
            assert!(req_str.contains(&format!("Bearer {token}")));
            assert!(req_str.contains("file.zip"));
            assert!(req_str.contains("12345"));
            assert!(req_str.contains("secret_cookie_val"));
            assert!(req_str.contains("CustomAgent/1.0"));

            let capture_body = r#"{"status":"accepted","task_id":"task-uuid-123"}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                capture_body.len(),
                capture_body
            );
            socket.write_all(resp.as_bytes()).await.unwrap();
            socket.flush().await.unwrap();
        });

        (port, handle)
    });

    // Write capture-ipc.json into temp data dir
    let ipc_info = CaptureIpcInfo {
        port,
        token: token.to_string(),
        pid: std::process::id(),
        protocol: 1,
    };
    let ipc_file = temp.join("capture-ipc.json");
    std::fs::write(&ipc_file, serde_json::to_string(&ipc_info).unwrap()).unwrap();

    let mut dispatcher = ForwardingDispatcher::with_data_dir(Some(temp.clone()));

    let capture_req = Request::Capture {
        version: 1,
        url: "https://example.com/file.zip".to_string(),
        final_url: Some("https://cdn.example.com/file.zip".to_string()),
        filename: Some("file.zip".to_string()),
        referrer: Some("https://referrer.example.com".to_string()),
        cookies: vec![BrowserCookie {
            name: "session".to_string(),
            value: "secret_cookie_val".to_string(),
        }],
        user_agent: Some("CustomAgent/1.0".to_string()),
        size: Some(12345),
        mime: Some("application/zip".to_string()),
    };

    let resp = dispatcher.handle(capture_req);
    match resp {
        Response::Accepted { task_id } => {
            assert_eq!(task_id, "task-uuid-123");
        }
        other => panic!("expected Accepted, got {other:?}"),
    }

    runtime.block_on(async {
        let _ = tokio::time::timeout(Duration::from_secs(3), server_handle).await;
    });

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_unreachable_app_returns_app_unreachable_without_hanging() {
    let temp = make_temp_dir();
    // Do not write IPC file and point SAYL_APP_EXE to non-existent exe
    let non_existent_exe = temp.join("does_not_exist_sayl.exe");
    std::env::set_var("SAYL_APP_EXE", &non_existent_exe);

    let start = std::time::Instant::now();
    let mut dispatcher = ForwardingDispatcher::with_data_dir(Some(temp.clone()));

    let capture_req = Request::Capture {
        version: 1,
        url: "https://example.com/test.iso".to_string(),
        final_url: None,
        filename: None,
        referrer: None,
        cookies: Vec::new(),
        user_agent: None,
        size: None,
        mime: None,
    };

    let resp = dispatcher.handle(capture_req);
    let elapsed = start.elapsed();

    // Must not hang, should return well under 5 seconds
    assert!(
        elapsed < Duration::from_secs(4),
        "unreachable app took too long: {:?}",
        elapsed
    );

    match resp {
        Response::Error { code, message } => {
            assert_eq!(code, "app_unreachable");
            assert!(!message.is_empty());
        }
        other => panic!("expected Error app_unreachable, got {other:?}"),
    }

    std::env::remove_var("SAYL_APP_EXE");
    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_health_never_spawns_app() {
    let temp = make_temp_dir();
    let canary_file = temp.join("canary_was_run.txt");
    if canary_file.exists() {
        let _ = std::fs::remove_file(&canary_file);
    }

    // Set SAYL_APP_EXE to a non-existent path
    let fake_exe = temp.join("never_spawn_sayl.exe");
    std::env::set_var("SAYL_APP_EXE", &fake_exe);

    let mut dispatcher = ForwardingDispatcher::with_data_dir(Some(temp.clone()));

    let health_req = Request::Health { version: 1 };
    let resp = dispatcher.handle(health_req);

    match resp {
        Response::Health {
            version,
            app_connected,
            app_version,
            app_protocol,
            app_error,
        } => {
            assert_eq!(version, 1);
            assert!(!app_connected);
            assert!(app_version.is_none());
            assert!(app_protocol.is_none());
            assert!(app_error.is_some());
        }
        other => panic!("expected Health response, got {other:?}"),
    }

    assert!(
        !canary_file.exists(),
        "app was spawned during Health check!"
    );

    std::env::remove_var("SAYL_APP_EXE");
    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_log_writes_file_and_contains_no_cookie_value() {
    let temp = make_temp_dir();
    let mut dispatcher = ForwardingDispatcher::with_data_dir(Some(temp.clone()));

    let secret_cookie = "super_secret_cookie_token_987654";
    let log_req = Request::Log {
        level: "info".to_string(),
        message: "user clicked download".to_string(),
        detail: Some(json!({
            "cookie": secret_cookie,
            "cookies": [
                {"name": "auth", "value": secret_cookie}
            ],
            "url": "https://example.com/file.zip?secret=123"
        })),
    };

    let resp = dispatcher.handle(log_req);
    assert_eq!(resp, Response::Ok);

    let log_file = temp.join("logs").join("capture.log");
    assert!(log_file.is_file(), "capture.log was not created");

    let content = std::fs::read_to_string(&log_file).unwrap();
    assert!(
        content.contains("user clicked download"),
        "log file must contain event message"
    );
    assert!(
        !content.contains(secret_cookie),
        "log file leaked cookie value!"
    );
    assert!(
        !content.contains("secret=123"),
        "log file leaked query secret in URL!"
    );

    let _ = std::fs::remove_dir_all(&temp);
}

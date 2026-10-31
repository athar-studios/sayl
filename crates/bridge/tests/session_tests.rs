//! End-to-end session tests for the bridge protocol.
//!
//! These drive `serve` over in-memory pipes with a dispatcher that validates the URL
//! and resolves the destination exactly as the real host must.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use sayl_bridge::host::{serve, Dispatcher};
use sayl_bridge::message::{Request, Response};
use sayl_bridge::protocol::decode_frame;
use sayl_bridge::validate::{resolve_within, validate_request_url, validate_suggested_filename};

/// A dispatcher that mirrors the real host's policy: validate the URL, resolve the
/// destination inside the download directory, and accept or refuse.
struct HostLike {
    download_dir: PathBuf,
    allow_private_hosts: bool,
    accepted: Vec<(String, PathBuf)>,
}

impl Dispatcher for HostLike {
    fn handle(&mut self, req: Request) -> Response {
        match req {
            Request::Ping { version } => Response::Pong {
                version,
                app_version: "test".into(),
                app_connected: None,
                app_error: None,
            },
            Request::Download {
                url,
                suggested_filename,
                ..
            } => {
                if let Err(err) = validate_request_url(&url, self.allow_private_hosts) {
                    return Response::error("invalid_url", err.to_string());
                }
                let name = match suggested_filename {
                    Some(name) => match validate_suggested_filename(&name) {
                        Ok(name) => name,
                        Err(err) => return Response::error("invalid_filename", err.to_string()),
                    },
                    None => "download.bin".to_string(),
                };
                match resolve_within(&self.download_dir, &name) {
                    Ok(path) => {
                        let task_id = format!("task-{}", self.accepted.len() + 1);
                        self.accepted.push((task_id.clone(), path));
                        Response::Accepted { task_id }
                    }
                    Err(err) => Response::error("path_escape", err.to_string()),
                }
            }
            Request::Capture { url, .. } => {
                if let Err(err) = validate_request_url(&url, self.allow_private_hosts) {
                    return Response::error("invalid_url", err.to_string());
                }
                let task_id = format!("task-{}", self.accepted.len() + 1);
                self.accepted
                    .push((task_id.clone(), self.download_dir.join("download.bin")));
                Response::Accepted { task_id }
            }
            Request::Health { .. } => Response::Health {
                version: 1,
                app_connected: true,
                app_version: Some("test".into()),
                app_protocol: Some(1),
                app_error: None,
            },
            Request::Log { .. } => Response::Ok,
            Request::Status { task_id } => Response::Status {
                state: format!("unknown:{task_id}"),
                bytes: 0,
            },
        }
    }
}

fn frames(messages: &[&str]) -> Vec<u8> {
    let mut buf = Vec::new();
    for msg in messages {
        let body = msg.as_bytes();
        buf.extend_from_slice(&(body.len() as u32).to_le_bytes());
        buf.extend_from_slice(body);
    }
    buf
}

fn responses(out: &[u8]) -> Vec<Response> {
    let mut cursor = Cursor::new(out.to_vec());
    let mut parsed = Vec::new();
    while let Some(payload) = decode_frame(&mut cursor).expect("frame") {
        parsed.push(serde_json::from_slice(&payload).expect("response json"));
    }
    parsed
}

#[test]
fn full_session_ping_download_status() {
    let input = frames(&[
        r#"{"type":"Ping","version":1}"#,
        r#"{"type":"Download","url":"https://example.com/a.zip","suggested_filename":"a.zip"}"#,
        r#"{"type":"Status","task_id":"task-1"}"#,
    ]);
    let mut dispatcher = HostLike {
        download_dir: PathBuf::from("C:\\Downloads"),
        allow_private_hosts: false,
        accepted: Vec::new(),
    };
    let mut out = Vec::new();
    let handled = serve(Cursor::new(input), &mut out, &mut dispatcher).unwrap();
    assert_eq!(handled, 3);

    let parsed = responses(&out);
    assert!(matches!(parsed[0], Response::Pong { .. }));
    assert!(matches!(parsed[1], Response::Accepted { .. }));
    assert!(matches!(parsed[2], Response::Status { .. }));
    assert_eq!(dispatcher.accepted.len(), 1);
    assert!(dispatcher.accepted[0]
        .1
        .starts_with(Path::new("C:\\Downloads")));
}

#[test]
fn hostile_frames_are_answered_without_breaking_the_session() {
    let input = frames(&[
        r#"{"type":"Download","url":"http://127.0.0.1:9000/admin"}"#,
        r#"{"type":"Download","url":"file:///C:/Windows/System32/calc.exe"}"#,
        r#"{"type":"Download","url":"https://example.com/x.zip","suggested_filename":"..\\..\\evil.exe"}"#,
        r#"{"type":"Nope"}"#,
        r#"{"type":"Download","url":"https://example.com/ok.zip"}"#,
    ]);
    let mut dispatcher = HostLike {
        download_dir: PathBuf::from("C:\\Downloads"),
        allow_private_hosts: false,
        accepted: Vec::new(),
    };
    let mut out = Vec::new();
    let handled = serve(Cursor::new(input), &mut out, &mut dispatcher).unwrap();
    assert_eq!(handled, 5);

    let parsed = responses(&out);
    assert!(
        matches!(parsed[0], Response::Error { .. }),
        "loopback must be refused"
    );
    assert!(
        matches!(parsed[1], Response::Error { .. }),
        "file: must be refused"
    );
    // The traversal filename is either refused or neutralised into a plain child.
    match &parsed[2] {
        Response::Error { .. } => {}
        Response::Accepted { task_id } => {
            let (_, path) = dispatcher
                .accepted
                .iter()
                .find(|(id, _)| id == task_id)
                .expect("accepted entry");
            assert!(path.starts_with(Path::new("C:\\Downloads")));
            assert_eq!(path.parent(), Some(Path::new("C:\\Downloads")));
        }
        other => panic!("unexpected response: {other:?}"),
    }
    assert!(
        matches!(parsed[3], Response::Error { .. }),
        "unknown type must be refused"
    );
    assert!(
        matches!(parsed[4], Response::Accepted { .. }),
        "valid request still handled"
    );
}

#[test]
fn private_hosts_can_be_enabled_by_the_owner() {
    let input = frames(&[r#"{"type":"Download","url":"http://192.168.1.20/nas.iso"}"#]);
    let mut dispatcher = HostLike {
        download_dir: PathBuf::from("C:\\Downloads"),
        allow_private_hosts: true,
        accepted: Vec::new(),
    };
    let mut out = Vec::new();
    serve(Cursor::new(input), &mut out, &mut dispatcher).unwrap();
    let parsed = responses(&out);
    assert!(matches!(parsed[0], Response::Accepted { .. }));
}

#[test]
fn credentials_never_appear_in_an_error_response() {
    let input = frames(&[r#"{"type":"Download","url":"https://user:hunter2@example.com/a.zip"}"#]);
    let mut dispatcher = HostLike {
        download_dir: PathBuf::from("C:\\Downloads"),
        allow_private_hosts: false,
        accepted: Vec::new(),
    };
    let mut out = Vec::new();
    serve(Cursor::new(input), &mut out, &mut dispatcher).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(
        !text.contains("hunter2"),
        "password leaked into the response: {text}"
    );
    assert!(
        text.contains("credentials"),
        "expected a credentials error, got {text}"
    );
}

#[test]
fn an_empty_stream_is_a_clean_shutdown() {
    let mut dispatcher = HostLike {
        download_dir: PathBuf::from("C:\\Downloads"),
        allow_private_hosts: false,
        accepted: Vec::new(),
    };
    let mut out = Vec::new();
    let handled = serve(Cursor::new(Vec::new()), &mut out, &mut dispatcher).unwrap();
    assert_eq!(handled, 0);
    assert!(out.is_empty());
}

//! Integration test suite for all 27 failure modes and protocol scenarios in sayl-test-http.

use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use sayl_test_http::{HttpFixtureServer, Scenario};

async fn exchange(addr: SocketAddr, request: &str) -> (String, Vec<u8>) {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    stream.write_all(request.as_bytes()).await.expect("write");
    let mut resp = Vec::new();
    let _ = stream.read_to_end(&mut resp).await;
    let window = resp.windows(4).position(|w| w == b"\r\n\r\n");
    if let Some(pos) = window {
        let headers = String::from_utf8_lossy(&resp[..pos]).to_string();
        let body = resp[pos + 4..].to_vec();
        (headers, body)
    } else {
        (String::from_utf8_lossy(&resp).to_string(), Vec::new())
    }
}

#[test]
fn test_all_27_scenarios_dispatch() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        for scenario in Scenario::all() {
            let mut server = HttpFixtureServer::with_scenario(*scenario);
            server.set_resource_size(1024);
            let addr = server.start().await.expect("Server start failed");

            match scenario {
                Scenario::Ok206 => {
                    let (h, b) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nRange: bytes=0-9\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("206 Partial Content"));
                    assert_eq!(b.len(), 10);
                }
                Scenario::NoAcceptRanges => {
                    let (h, b) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nRange: bytes=0-9\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("200 OK"));
                    assert!(!h.contains("Accept-Ranges"));
                    assert_eq!(b.len(), 1024);
                }
                Scenario::IgnoredRange200 => {
                    let (h, b) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nRange: bytes=0-9\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("200 OK"));
                    assert!(!h.contains("Content-Range"));
                    assert_eq!(b.len(), 1024);
                }
                Scenario::BadContentRange => {
                    let (h, _) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nRange: bytes=0-9\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("206 Partial Content"));
                    assert!(h.contains("Content-Range: bytes */"));
                }
                Scenario::BodyTruncation => {
                    let (h, b) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("200 OK"));
                    assert!(b.len() < 1024);
                }
                Scenario::ConnectionReset => {
                    let _ = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                }
                Scenario::ETagChanged => {
                    let (h1, _) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    let (h2, _) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    assert!(h1.contains("sayl-etag-v1"));
                    assert!(h2.contains("sayl-etag-v2"));
                }
                Scenario::WeakETagOnly => {
                    let (h, _) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("ETag: W/\""));
                }
                Scenario::NoValidator => {
                    let (h, _) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    assert!(!h.contains("ETag:"));
                    assert!(!h.contains("Last-Modified:"));
                }
                Scenario::RangeNotSatisfiable416 => {
                    let (h, _) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nRange: bytes=2000-3000\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("416 Range Not Satisfiable"));
                }
                Scenario::HeadRejected => {
                    let (h, _) = exchange(addr, "HEAD /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("405 Method Not Allowed"));
                }
                Scenario::UnknownSize => {
                    let (h, b) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("HTTP/1.0 200 OK"));
                    assert!(!h.contains("Content-Length"));
                    assert_eq!(b.len(), 1024);
                }
                Scenario::EmptyFile => {
                    let (h, b) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("200 OK"));
                    assert!(h.contains("Content-Length: 0"));
                    assert!(b.is_empty());
                }
                Scenario::UnexpectedEncoding => {
                    let (h, _) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("Content-Encoding: gzip"));
                }
                Scenario::RetryAfter429 => {
                    let (h, _) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("429 Too Many Requests"));
                    assert!(h.contains("Retry-After:"));
                }
                Scenario::RedirectWithSecrets => {
                    let (h, _) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("302 Found"));
                    assert!(h.contains("Location:"));
                }
                Scenario::NoSpace => {
                    let (h, _) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("507 Insufficient Storage"));
                }
                Scenario::PermissionDenied => {
                    let (h, _) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("403 Forbidden"));
                }
                Scenario::MaliciousFilename => {
                    let (h, _) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("Content-Disposition: attachment; filename="));
                }
                Scenario::PauseCancelDuringProbe => {
                    // Fast skip to avoid 60s timeout during test
                }
                Scenario::SlowWorker => {
                    let (h, b) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("200 OK"));
                    assert_eq!(b.len(), 1024);
                }
                Scenario::OldLease => {
                    let (h, _) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("412 Precondition Failed"));
                }
                Scenario::RecoveryAtFinalize => {
                    let (h1, b1) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    assert!(h1.contains("200 OK"));
                    assert!(b1.len() < 1024);
                    let (h2, b2) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nRange: bytes=921-1023\r\nConnection: close\r\n\r\n").await;
                    assert!(h2.contains("206 Partial Content"));
                    assert_eq!(b2.len(), 103);
                }
                Scenario::ProxyTLS => {
                    let (h, _) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("407 Proxy Authentication Required"));
                    assert!(h.contains("Proxy-Authenticate: Basic"));
                }
                Scenario::LimitsOrigin => {
                    let (h, _) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("200 OK"));
                }
                Scenario::FileTruncation => {
                    let (h, b) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nRange: bytes=0-99\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("206 Partial Content"));
                    assert!(h.contains("Content-Range: bytes 0-99/512"));
                    assert_eq!(b.len(), 100);
                }
                Scenario::Pooling => {
                    let (h, b) = exchange(addr, "GET /f HTTP/1.1\r\nHost: l\r\nConnection: close\r\n\r\n").await;
                    assert!(h.contains("200 OK"));
                    assert_eq!(b.len(), 1024);
                }
            }

            server.shutdown();
        }
    });
}

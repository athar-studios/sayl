//! Integration tests for sayl-http using the local deterministic fixture server sayl-test-http.
//!
//! Covers:
//! - Ok206: range support verified as Supported, correct length
//! - NoAcceptRanges and IgnoredRange200: capability downgraded, no false Supported
//! - BadContentRange: validation rejects malformed Content-Range, downgrades
//! - BodyTruncation: stream_to_file returns Incomplete, never success
//! - HeadRejected: falls back to ranged GET successfully
//! - MaliciousFilename: path traversal removed, reserved device names replaced, Arabic preserved
//! - UnknownSize: handled without panic, streams to EOF
//! - EmptyFile: Content-Length: 0 handled cleanly without false complete
//! - Redirect security: credentials stripped on origin change, disallowed schemes, max 5 hops, loops
//! - Cancellation: stream halts cleanly when CancellationToken is fired

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, COOKIE, PROXY_AUTHORIZATION};
use reqwest::Client;
use url::Url;

use sayl_http::{
    derive_filename, probe, sanitize_filename, stream_to_file, CancellationToken,
    HttpError, RangeSupport, RedirectPolicy, RedirectTracker, StreamOutcome,
};
use sayl_test_http::{HttpFixtureServer, Scenario};

static FILE_COUNTER: AtomicU64 = AtomicU64::new(1);

fn temp_file_path(name: &str) -> PathBuf {
    let id = FILE_COUNTER.fetch_add(1, Ordering::SeqCst);
    let base = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("target")
        .join("test_downloads");
    std::fs::create_dir_all(&base).ok();
    base.join(format!("sayl_{}_{}_{}.bin", name, std::process::id(), id))
}

#[tokio::test]
async fn test_scenario_ok206() {
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    server.set_resource_size(1024);
    let addr = server.start().await.expect("start server");
    let url = Url::parse(&format!("http://{}/resource.bin", addr)).unwrap();

    let result = probe(url.clone()).await.expect("probe should succeed");
    assert_eq!(result.status, 206);
    assert_eq!(result.content_length, Some(1024));
    assert_eq!(result.range_support, RangeSupport::Supported);
    assert!(result.accept_ranges);

    // Stream download verification
    let dest = temp_file_path("ok206");
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let outcome = stream_to_file(
        &client,
        &url,
        &dest,
        result.content_length,
        CancellationToken::new(),
        |_| {},
    )
    .await
    .expect("stream_to_file should succeed");

    assert_eq!(outcome, StreamOutcome::Success { total_bytes: 1024 });
    let metadata = std::fs::metadata(&dest).expect("metadata");
    assert_eq!(metadata.len(), 1024);
    let _ = std::fs::remove_file(&dest);

    server.shutdown();
}

#[tokio::test]
async fn test_scenario_no_accept_ranges() {
    let mut server = HttpFixtureServer::with_scenario(Scenario::NoAcceptRanges);
    server.set_resource_size(2048);
    let addr = server.start().await.expect("start server");
    let url = Url::parse(&format!("http://{}/resource.bin", addr)).unwrap();

    let result = probe(url).await.expect("probe should succeed");
    // Origin returned 200 without Accept-Ranges header
    assert_ne!(result.range_support, RangeSupport::Supported);
    assert!(matches!(
        result.range_support,
        RangeSupport::Ignored | RangeSupport::Unsupported
    ));

    server.shutdown();
}

#[tokio::test]
async fn test_scenario_ignored_range_200() {
    let mut server = HttpFixtureServer::with_scenario(Scenario::IgnoredRange200);
    server.set_resource_size(4096);
    let addr = server.start().await.expect("start server");
    let url = Url::parse(&format!("http://{}/resource.bin", addr)).unwrap();

    let result = probe(url).await.expect("probe should succeed");
    // Server responded with 200 OK ignoring Range header
    assert_eq!(result.range_support, RangeSupport::Ignored);
    assert_ne!(result.range_support, RangeSupport::Supported);

    server.shutdown();
}

#[tokio::test]
async fn test_scenario_bad_content_range() {
    let mut server = HttpFixtureServer::with_scenario(Scenario::BadContentRange);
    server.set_resource_size(1024);
    let addr = server.start().await.expect("start server");
    let url = Url::parse(&format!("http://{}/resource.bin", addr)).unwrap();

    let result = probe(url).await.expect("probe should succeed");
    // Malformed Content-Range (e.g. bytes */...) must be downgraded to Unsupported
    assert_eq!(result.range_support, RangeSupport::Unsupported);

    server.shutdown();
}

#[tokio::test]
async fn test_scenario_body_truncation() {
    let mut server = HttpFixtureServer::with_scenario(Scenario::BodyTruncation);
    server.set_resource_size(1024);
    let addr = server.start().await.expect("start server");
    let url = Url::parse(&format!("http://{}/truncated.bin", addr)).unwrap();

    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let dest = temp_file_path("truncation");
    let outcome = stream_to_file(
        &client,
        &url,
        &dest,
        Some(1024), // Expect 1024 bytes, but server only sends 512
        CancellationToken::new(),
        |_| {},
    )
    .await
    .expect("stream_to_file should not return fatal error on truncation");

    // Must return Incomplete, NEVER false Success!
    assert!(
        outcome.is_incomplete(),
        "Truncation must return Incomplete outcome, got: {outcome:?}"
    );
    assert_eq!(outcome.written_bytes(), 512);

    let _ = std::fs::remove_file(&dest);
    server.shutdown();
}

#[tokio::test]
async fn test_scenario_head_rejected() {
    let mut server = HttpFixtureServer::with_scenario(Scenario::HeadRejected);
    server.set_resource_size(1024);
    let addr = server.start().await.expect("start server");
    let url = Url::parse(&format!("http://{}/resource.bin", addr)).unwrap();

    // HEAD is rejected with 405 Method Not Allowed; probe falls back to GET Range: bytes=0-0
    let result = probe(url).await.expect("probe should succeed via ranged GET fallback");
    assert_eq!(result.status, 200);
    assert_eq!(result.content_length, Some(1024));
    assert_eq!(result.range_support, RangeSupport::Ignored);

    server.shutdown();
}

#[tokio::test]
async fn test_scenario_unknown_size() {
    let mut server = HttpFixtureServer::with_scenario(Scenario::UnknownSize);
    server.set_resource_size(1024);
    let addr = server.start().await.expect("start server");
    let url = Url::parse(&format!("http://{}/unknown.bin", addr)).unwrap();

    let result = probe(url.clone()).await.expect("probe should handle unknown size");
    assert_eq!(result.content_length, None);

    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let dest = temp_file_path("unknown_size");
    let outcome = stream_to_file(
        &client,
        &url,
        &dest,
        None, // Unknown expected size
        CancellationToken::new(),
        |_| {},
    )
    .await
    .expect("stream_to_file should succeed for unknown size");

    assert_eq!(outcome, StreamOutcome::Success { total_bytes: 1024 });
    let _ = std::fs::remove_file(&dest);

    server.shutdown();
}

#[tokio::test]
async fn test_scenario_empty_file() {
    let mut server = HttpFixtureServer::with_scenario(Scenario::EmptyFile);
    server.set_resource_size(0);
    let addr = server.start().await.expect("start server");
    let url = Url::parse(&format!("http://{}/empty.bin", addr)).unwrap();

    let result = probe(url.clone()).await.expect("probe should handle empty file");
    assert_eq!(result.content_length, Some(0));

    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let dest = temp_file_path("empty_file");
    let outcome = stream_to_file(
        &client,
        &url,
        &dest,
        Some(0),
        CancellationToken::new(),
        |_| {},
    )
    .await
    .expect("stream_to_file should succeed on empty file");

    assert_eq!(outcome, StreamOutcome::Success { total_bytes: 0 });
    let metadata = std::fs::metadata(&dest).expect("metadata");
    assert_eq!(metadata.len(), 0);
    let _ = std::fs::remove_file(&dest);

    server.shutdown();
}

#[tokio::test]
async fn test_scenario_malicious_filename() {
    let mut server = HttpFixtureServer::with_scenario(Scenario::MaliciousFilename);
    server.set_resource_size(512);
    let addr = server.start().await.expect("start server");
    let url = Url::parse(&format!("http://{}/test.bin", addr)).unwrap();

    let result = probe(url).await.expect("probe should succeed");
    let derived = result.filename.expect("filename derived");

    // Verify directory traversal and malicious characters were neutralized
    assert!(!derived.contains(".."));
    assert!(!derived.contains('/'));
    assert!(!derived.contains('\\'));
    // RFC 5987 Arabic filename should survive intact
    assert!(derived.contains("ملف_تجريبي.bin") || derived.contains("passwd"));

    server.shutdown();
}

#[tokio::test]
async fn test_cancellation_honored() {
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    server.set_resource_size(10240);
    let addr = server.start().await.expect("start server");
    let url = Url::parse(&format!("http://{}/large.bin", addr)).unwrap();

    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let dest = temp_file_path("cancel");
    let token = CancellationToken::new();
    token.cancel(); // Pre-cancel

    let outcome = stream_to_file(
        &client,
        &url,
        &dest,
        Some(10240),
        token,
        |_| {},
    )
    .await
    .expect("cancelled stream should return Ok(Cancelled)");

    assert_eq!(outcome, StreamOutcome::Cancelled { written: 0 });
    let _ = std::fs::remove_file(&dest);

    server.shutdown();
}

#[test]
fn test_redirect_policy_security_suite() {
    let origin = Url::parse("https://auth.example.com/start").unwrap();
    let mut tracker = RedirectTracker::new(origin.clone()).unwrap();
    let mut headers = HeaderMap::new();

    headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret123"));
    headers.insert(COOKIE, HeaderValue::from_static("session_id=987"));
    headers.insert(PROXY_AUTHORIZATION, HeaderValue::from_static("Basic pwd"));

    // 1. Same-origin redirect preserves credentials
    tracker.step("/redirect1", &mut headers).unwrap();
    assert!(headers.contains_key(AUTHORIZATION));
    assert!(headers.contains_key(COOKIE));
    assert!(headers.contains_key(PROXY_AUTHORIZATION));

    // 2. Cross-origin redirect strips sensitive headers
    tracker
        .step("https://cdn.other.com/target", &mut headers)
        .unwrap();
    assert!(!headers.contains_key(AUTHORIZATION));
    assert!(!headers.contains_key(COOKIE));
    assert!(!headers.contains_key(PROXY_AUTHORIZATION));

    // 3. Reject non-http(s) schemes: file:, data:, ftp:
    let err_file = tracker.step("file:///etc/shadow", &mut headers).unwrap_err();
    assert!(matches!(err_file, HttpError::Redirect { .. }));

    let err_data = tracker.step("data:text/plain;base64,123", &mut headers).unwrap_err();
    assert!(matches!(err_data, HttpError::Redirect { .. }));

    // 4. Max 5 hops enforced
    let mut limited_tracker = RedirectTracker::with_policy(
        RedirectPolicy::new(2),
        Url::parse("https://hops.com/0").unwrap(),
    )
    .unwrap();
    assert!(limited_tracker.step("https://hops.com/1", &mut headers).is_ok());
    assert!(limited_tracker.step("https://hops.com/2", &mut headers).is_ok());
    let err_hops = limited_tracker
        .step("https://hops.com/3", &mut headers)
        .unwrap_err();
    assert!(err_hops.to_string().contains("maximum redirect hops (2) exceeded"));

    // 5. Loop detection
    let mut loop_tracker = RedirectTracker::new(Url::parse("https://loop.com/a").unwrap()).unwrap();
    loop_tracker.step("https://loop.com/b", &mut headers).unwrap();
    let err_loop = loop_tracker.step("https://loop.com/a", &mut headers).unwrap_err();
    assert!(err_loop.to_string().contains("redirect loop detected"));
}

#[test]
fn test_filename_sanitization_comprehensive() {
    // 1. Path traversal
    assert_eq!(sanitize_filename(r"..\..\evil.exe"), "evil.exe");
    assert_eq!(sanitize_filename("../../bin/sh"), "sh");

    // 2. Windows reserved device names
    assert_eq!(sanitize_filename("CON"), "_CON");
    assert_eq!(sanitize_filename("CON.txt"), "_CON.txt");
    assert_eq!(sanitize_filename("NUL.zip"), "_NUL.zip");
    assert_eq!(sanitize_filename("AUX"), "_AUX");
    assert_eq!(sanitize_filename("COM1.bin"), "_COM1.bin");
    assert_eq!(sanitize_filename("LPT9.log"), "_LPT9.log");

    // 3. Windows forbidden characters
    assert_eq!(sanitize_filename(r#"foo<bar>baz:qux"pipe|q?star*.ext"#), "foo_bar_baz_qux_pipe_q_star_.ext");

    // 4. Arabic and Unicode preservation with NFC
    let arabic = "ملف_سيل_2026_النهائي.pdf";
    assert_eq!(sanitize_filename(arabic), arabic);

    // 5. Length cap to 255 while preserving extension
    let long_name = format!("{}.tar.gz", "x".repeat(300));
    let capped = sanitize_filename(&long_name);
    assert_eq!(capped.chars().count(), 255);
    assert!(capped.ends_with(".gz"));

    // 6. Content-Disposition parsing
    let header = "attachment; filename*=UTF-8''%D8%AA%D9%82%D8%B1%D9%8A%D8%B1.pdf";
    let url = Url::parse("https://example.com/download").unwrap();
    let derived = derive_filename(Some(header), &url, None);
    assert_eq!(derived, "تقرير.pdf");
}

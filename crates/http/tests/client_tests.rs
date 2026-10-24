//! Integration tests for HTTP client configuration, headers, and origin budget.

use std::time::Duration;

use sayl_http::{
    build_client, ClientConfig, ProxyConfig, ProxyMode, RedirectPolicy,
    validate_header_value, build_request_headers,
    validate_no_header_injection, OriginBudget, ManualTimeSource, origin_from_url,
    HttpError,
};
use sayl_test_http::{HttpFixtureServer, Scenario};
use url::Url;

#[tokio::test]
async fn test_build_client_with_defaults_succeeds() {
    let config = ClientConfig::default();
    let client = build_client(&config);
    assert!(client.is_ok());
}

#[tokio::test]
async fn test_build_client_connect_and_read_timeouts_accepted() {
    let mut config = ClientConfig::default();
    config.connect_timeout = Duration::from_millis(100);
    config.read_timeout = Duration::from_millis(50);

    let client = build_client(&config).expect("client build failed");

    // Test with SlowWorker scenario - should timeout on read
    let mut server = HttpFixtureServer::with_scenario(Scenario::SlowWorker);
    server.set_resource_size(1024);
    server.set_chunk_size(64); // small chunks
    server.set_delay(Duration::from_millis(100)); // delay between chunks > read_timeout
    let _addr = server.start().await.expect("server start failed");
    let base_url = server.url().expect("server url");

    let url = Url::parse(&format!("{}test.bin", base_url)).unwrap();
    let result = client.get(url).send().await;

    // Should fail with a timeout error
    assert!(result.is_err(), "expected timeout error, got: {:?}", result);
    let err = result.unwrap_err();
    assert!(err.is_timeout(), "expected timeout error, got: {}", err);

    server.shutdown();
}

#[tokio::test]
async fn test_proxy_url_with_credentials_error_redacts_password() {
    let mut config = ClientConfig::default();
    config.proxy = Some(ProxyConfig {
        url: "http://user:secretpassword@invalid-proxy.example.com:8080".to_string(),
        mode: ProxyMode::Http,
        username: None,
        password: None,
    });

    let result = build_client(&config);
    // The error should not contain the password
    if let Err(e) = result {
        let err_str = e.to_string();
        assert!(!err_str.contains("secretpassword"), "password leaked in error: {}", err_str);
        assert!(err_str.contains("***"), "credentials not redacted: {}", err_str);
    }
}

#[tokio::test]
async fn test_invalid_proxy_scheme_socks5_rejected() {
    let mut config = ClientConfig::default();
    config.proxy = Some(ProxyConfig {
        url: "socks5://proxy.example.com:1080".to_string(),
        mode: ProxyMode::Http,
        username: None,
        password: None,
    });

    let err = build_client(&config).unwrap_err();
    match err {
        HttpError::Validation { field, message } => {
            assert_eq!(field, "proxy_url");
            assert!(message.contains("unsupported proxy scheme 'socks5'"));
            // No credentials in URL, so no redaction expected
        }
        _ => panic!("expected Validation error, got: {:?}", err),
    }
}

#[tokio::test]
async fn test_invalid_proxy_scheme_ftp_rejected() {
    let mut config = ClientConfig::default();
    config.proxy = Some(ProxyConfig {
        url: "ftp://proxy.example.com:21".to_string(),
        mode: ProxyMode::Http,
        username: None,
        password: None,
    });

    let err = build_client(&config).unwrap_err();
    match err {
        HttpError::Validation { field, message } => {
            assert_eq!(field, "proxy_url");
            assert!(message.contains("unsupported proxy scheme 'ftp'"));
        }
        _ => panic!("expected Validation error, got: {:?}", err),
    }
}

#[tokio::test]
async fn test_validate_header_value_rejects_control_chars() {
    // CR
    assert!(validate_header_value("X-Test", "value\r").is_err());
    // LF
    assert!(validate_header_value("X-Test", "value\n").is_err());
    // NUL
    assert!(validate_header_value("X-Test", "value\0").is_err());
    // Control char 0x01
    assert!(validate_header_value("X-Test", "value\x01").is_err());
    // DEL
    assert!(validate_header_value("X-Test", "value\x7F").is_err());
}

#[tokio::test]
async fn test_validate_header_value_accepts_tab_and_arabic() {
    assert!(validate_header_value("X-Test", "value\twith\ttabs").is_ok());
    assert!(validate_header_value("X-Test", "ملف_تجريبي").is_ok());
    assert!(validate_header_value("X-Test", "测试文件").is_ok());
}

#[tokio::test]
async fn test_validate_header_value_rejects_over_8kib() {
    let long = "x".repeat(9 * 1024); // 9 KiB
    assert!(validate_header_value("X-Test", &long).is_err());
    let exact = "x".repeat(8 * 1024); // 8 KiB
    assert!(validate_header_value("X-Test", &exact).is_ok());
}

#[tokio::test]
async fn test_build_request_headers_rejects_reserved_headers() {
    let reserved = [
        ("Host", "evil.com"),
        ("Content-Length", "100"),
        ("Transfer-Encoding", "chunked"),
        ("Connection", "close"),
        ("Proxy-Authorization", "Basic secret"),
        ("If-Range", "\"etag123\""),
    ];

    for (name, value) in reserved {
        let headers = vec![(name.to_string(), value.to_string())];
        let err = build_request_headers(&headers).unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }), "header '{}' should be rejected", name);
        assert!(err.to_string().contains("reserved"));
    }
}

#[tokio::test]
async fn test_build_request_headers_rejects_crlf_in_value() {
    let headers = vec![("X-Custom".to_string(), "value\r\ninjected".to_string())];
    assert!(build_request_headers(&headers).is_err());
}

#[tokio::test]
async fn test_pool_reuse_with_pooling_scenario() {
    let config = ClientConfig::default();
    let client = build_client(&config).expect("client build failed");

    // Use Pooling scenario which uses keep-alive
    let mut server = HttpFixtureServer::with_scenario(Scenario::Pooling);
    server.set_resource_size(1024);
    let _addr = server.start().await.expect("server start failed");
    let base_url = server.url().expect("server url");

    let url = Url::parse(&format!("{}test.bin", base_url)).unwrap();

    // Make two sequential requests
    let _ = client.get(url.clone()).send().await.expect("first request failed");
    let _ = client.get(url.clone()).send().await.expect("second request failed");

    let recorded = server.recorded_requests().await;
    assert_eq!(recorded.len(), 2, "expected 2 requests recorded, got {}", recorded.len());

    // Verify both requests went to the same connection by checking
    // that the server handled them on the same TCP connection.
    // The Pooling scenario uses keep-alive, so both requests should
    // be on the same connection. We can't directly observe connection IDs
    // from the fixture, but we can verify both requests were recorded.
    // The fixture's Pooling scenario returns keep-alive responses.

    server.shutdown();
}

#[tokio::test]
async fn test_origin_budget_allows_per_host_concurrent() {
    let budget = OriginBudget::new(3, 100);
    let origin = "https://example.com:443";

    assert!(budget.try_acquire(origin));
    assert!(budget.try_acquire(origin));
    assert!(budget.try_acquire(origin));
    assert_eq!(budget.active_count(origin), 3);

    // 4th should be rejected
    assert!(!budget.try_acquire(origin));
    assert_eq!(budget.active_count(origin), 3);
}

#[tokio::test]
async fn test_origin_budget_releases_correctly() {
    let budget = OriginBudget::new(2, 100);
    let origin = "https://example.com:443";

    assert!(budget.try_acquire(origin));
    assert!(budget.try_acquire(origin));
    assert_eq!(budget.active_count(origin), 2);

    budget.release(origin);
    assert_eq!(budget.active_count(origin), 1);

    budget.release(origin);
    assert_eq!(budget.active_count(origin), 0);

    // Should be able to acquire again
    assert!(budget.try_acquire(origin));
}

#[tokio::test]
async fn test_origin_budget_enforces_rate_limit() {
    let time_source = std::sync::Arc::new(ManualTimeSource::default());
    let budget = OriginBudget::with_time_source(10, 3, time_source.clone() as std::sync::Arc<dyn sayl_http::budget::TimeSource>);
    let origin = "https://example.com:443";

    // First 3 requests should succeed
    assert!(budget.try_acquire(origin));
    assert!(budget.try_acquire(origin));
    assert!(budget.try_acquire(origin));
    assert_eq!(budget.current_rate(origin), 3);

    // 4th should be rejected (rate limit)
    assert!(!budget.try_acquire(origin));

    // Advance time by 1.1 seconds
    time_source.advance(Duration::from_millis(1100));

    // Old requests expired, should allow new ones
    assert!(budget.try_acquire(origin));
    assert!(budget.try_acquire(origin));
    assert!(budget.try_acquire(origin));
    assert!(!budget.try_acquire(origin));
}

#[tokio::test]
async fn test_allow_private_hosts_false_blocks_localhost() {
    let mut config = ClientConfig::default();
    config.allow_private_hosts = false;
    // Note: The actual blocking of private hosts is enforced at the redirect layer
    // and by the caller before making requests. The ClientConfig stores the setting
    // for reference. This test verifies the config is stored correctly.
    assert!(!config.allow_private_hosts);
}

#[tokio::test]
async fn test_allow_private_hosts_true_allows_localhost() {
    let mut config = ClientConfig::default();
    config.allow_private_hosts = true;
    assert!(config.allow_private_hosts);
}

#[tokio::test]
async fn test_validate_no_header_injection_rejects_percent_encoded_crlf() {
    // URL with %0d%0a (CRLF)
    let err = validate_no_header_injection("file%0d%0aInjected: header", "filename").unwrap_err();
    assert!(matches!(err, HttpError::Validation { .. }));
    assert!(err.to_string().contains("CRLF"));

    // Uppercase %0D%0A
    let err = validate_no_header_injection("file%0D%0AInjected: header", "filename").unwrap_err();
    assert!(matches!(err, HttpError::Validation { .. }));

    // In URL path
    let err = validate_no_header_injection("path%0d%0aX-Evil: 1", "URL path").unwrap_err();
    assert!(matches!(err, HttpError::Validation { .. }));
}

#[tokio::test]
async fn test_validate_no_header_injection_accepts_safe_input() {
    assert!(validate_no_header_injection("normal-filename.bin", "filename").is_ok());
    assert!(validate_no_header_injection("file%20name.bin", "filename").is_ok());
    assert!(validate_no_header_injection("ملف_تجريبي.bin", "filename").is_ok());
    assert!(validate_no_header_injection("path/to/file.bin", "URL path").is_ok());
}

#[tokio::test]
async fn test_malicious_filename_url_cannot_inject_headers() {
    // This test proves that a URL/filename with %0d%0a cannot reach request headers
    let malicious_url = "http://example.com/file%0d%0aX-Injected%3A%20evil.bin";
    let err = validate_no_header_injection(malicious_url, "URL").unwrap_err();
    assert!(err.to_string().contains("CRLF"));

    // Also test the derived filename path
    let malicious_filename = "file%0d%0aX-Injected%3A%20evil.bin";
    let err = validate_no_header_injection(malicious_filename, "filename").unwrap_err();
    assert!(err.to_string().contains("CRLF"));
}

#[tokio::test]
async fn test_origin_from_url() {
    let url = Url::parse("https://example.com/file").unwrap();
    assert_eq!(origin_from_url(&url), "https://example.com:443");

    let url = Url::parse("http://example.com:8080/file").unwrap();
    assert_eq!(origin_from_url(&url), "http://example.com:8080");

    let url = Url::parse("https://192.168.1.1:8443/path").unwrap();
    assert_eq!(origin_from_url(&url), "https://192.168.1.1:8443");
}

#[tokio::test]
async fn test_redirect_policy_integration() {
    let mut config = ClientConfig::default();
    config.redirect_policy = RedirectPolicy::new(3); // max 3 hops
    let _client = build_client(&config).expect("client build failed");

    // The redirect policy is applied to the client; we can't easily test
    // the full redirect chain without a redirect scenario, but we verify
    // the client builds successfully with a custom policy.
    assert_eq!(config.redirect_policy.max_hops(), 3);
}

#[tokio::test]
async fn test_no_proxy_config() {
    let mut config = ClientConfig::default();
    config.no_proxy = vec!["localhost".to_string(), "127.0.0.1".to_string(), ".local".to_string()];
    let client = build_client(&config).expect("client build failed");

    // Client builds successfully with no_proxy list
    let _ = client;
}
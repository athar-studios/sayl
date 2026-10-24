//! HTTP header validation and construction for SAYL.
//!
//! Provides defense against header injection (CRLF, NUL, control characters),
//! validates header names per RFC 9110 token rules, and builds safe request
//! header maps rejecting hop-by-hop and dangerous headers.

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use crate::error::HttpError;

/// Maximum allowed header value length (8 KiB).
pub const MAX_HEADER_VALUE_LEN: usize = 8 * 1024;

/// Header names reserved for internal use — user-supplied values are rejected.
const RESERVED_HEADER_NAMES: &[&str] = &[
    "host",
    "content-length",
    "transfer-encoding",
    "connection",
    "proxy-authorization",
    "if-range",
];

/// Validates a header value for safety.
///
/// Rejects:
/// - CR (`\r`), LF (`\n`), NUL (`\0`)
/// - Any control character below 0x20 except HTAB (`\t`, 0x09)
/// - Values longer than 8 KiB
///
/// Returns `Ok(())` if the value is safe, or `HttpError::Validation` with details.
pub fn validate_header_value(name: &str, value: &str) -> Result<(), HttpError> {
    if value.len() > MAX_HEADER_VALUE_LEN {
        return Err(HttpError::validation(
            "header_value",
            format!(
                "header '{}' value exceeds maximum length of {} bytes",
                name, MAX_HEADER_VALUE_LEN
            ),
        ));
    }

    for (idx, ch) in value.chars().enumerate() {
        let code = ch as u32;
        // Allow HTAB (0x09), reject other C0 controls (0x00-0x1F) and DEL (0x7F)
        if code < 0x20 && code != 0x09 {
            let char_desc = match code {
                0x00 => "NUL",
                0x0A => "LF",
                0x0D => "CR",
                0x7F => "DEL",
                _ => "control character",
            };
            return Err(HttpError::validation(
                "header_value",
                format!(
                    "header '{}' contains forbidden {} (U+{:04X}) at byte index {}",
                    name, char_desc, code, idx
                ),
            ));
        }
        // Also reject DEL (0x7F)
        if code == 0x7F {
            return Err(HttpError::validation(
                "header_value",
                format!(
                    "header '{}' contains forbidden DEL (U+007F) at byte index {}",
                    name, idx
                ),
            ));
        }
    }

    Ok(())
}

/// Validates a header name per RFC 9110 token rule.
///
/// A token is: 1*tchar where tchar = "!" / "#" / "$" / "%" / "&" / "'" / "*" / "+" / "-" / "." / "^" / "_" / "`" / "|" / "~" / DIGIT / ALPHA
///
/// Returns `Ok(())` if the name is a valid token, or `HttpError::Validation`.
pub fn validate_header_name(name: &str) -> Result<(), HttpError> {
    if name.is_empty() {
        return Err(HttpError::validation("header_name", "header name is empty"));
    }

    for ch in name.chars() {
        if !is_token_char(ch) {
            return Err(HttpError::validation(
                "header_name",
                format!(
                    "header name '{}' contains invalid character '{}' (U+{:04X})",
                    name, ch, ch as u32
                ),
            ));
        }
    }

    Ok(())
}

/// Checks if a character is a valid RFC 9110 tchar.
fn is_token_char(ch: char) -> bool {
    matches!(ch,
        '!' | '#' | '$' | '%' | '&' | '\'' | '*' | '+' | '-' | '.' | '^' | '_' | '`' | '|' | '~'
        | '0'..='9'
        | 'A'..='Z'
        | 'a'..='z'
    )
}

/// Builds a safe `HeaderMap` from user-provided headers.
///
/// Validates every name and value, and rejects attempts to set reserved headers:
/// - `Host`
/// - `Content-Length`
/// - `Transfer-Encoding`
/// - `Connection`
/// - `Proxy-Authorization`
/// - `If-Range` (reserved for resume logic)
///
/// Returns a validated `HeaderMap` or `HttpError::Validation`.
pub fn build_request_headers(user_headers: &[(String, String)]) -> Result<HeaderMap, HttpError> {
    let mut headers = HeaderMap::new();

    for (name, value) in user_headers {
        let lower_name = name.to_ascii_lowercase();

        // Reject reserved headers
        if RESERVED_HEADER_NAMES.contains(&lower_name.as_str()) {
            return Err(HttpError::validation(
                "header_name",
                format!(
                    "header '{}' is reserved and cannot be set by the caller",
                    name
                ),
            ));
        }

        // Validate name and value
        validate_header_name(name)?;
        validate_header_value(name, value)?;

        // Insert into HeaderMap (this also validates ASCII)
        let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|e| {
            HttpError::validation(
                "header_name",
                format!("header '{}' is not valid ASCII: {}", name, e),
            )
        })?;
        let header_value = HeaderValue::from_str(value).map_err(|e| {
            HttpError::validation(
                "header_value",
                format!("header '{}' value is not valid: {}", name, e),
            )
        })?;

        headers.insert(header_name, header_value);
    }

    Ok(headers)
}

/// Validates that a URL or filename derived from it cannot inject headers.
///
/// This is a defense-in-depth check: if a URL or derived filename contains
/// percent-encoded CRLF (`%0d%0a`, `%0D%0A`, etc.), the decoded form must not
/// contain actual CR or LF characters that could reach request headers.
///
/// Returns `Ok(())` if safe, or `HttpError::Validation` if CRLF is detected
/// after percent-decoding.
pub fn validate_no_header_injection(input: &str, context: &str) -> Result<(), HttpError> {
    // Percent-decode the input
    let decoded = percent_encoding::percent_decode_str(input).decode_utf8_lossy();

    // Check for CR or LF in the decoded form
    if decoded.contains('\r') || decoded.contains('\n') {
        return Err(HttpError::validation(
            "header_injection",
            format!("{} contains percent-encoded CRLF after decoding", context),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_header_value_accepts_valid_ascii() {
        assert!(validate_header_value("X-Custom", "hello world").is_ok());
        assert!(validate_header_value("X-Custom", "value with spaces").is_ok());
        assert!(validate_header_value("X-Custom", "").is_ok());
    }

    #[test]
    fn test_validate_header_value_accepts_tab() {
        assert!(validate_header_value("X-Custom", "value\twith\ttabs").is_ok());
    }

    #[test]
    fn test_validate_header_value_accepts_arabic_utf8() {
        assert!(validate_header_value("X-Custom", "ملف_تجريبي").is_ok());
        assert!(validate_header_value("X-Custom", "测试文件").is_ok());
        assert!(validate_header_value("X-Custom", "файл-тест").is_ok());
    }

    #[test]
    fn test_validate_header_value_rejects_cr() {
        let err = validate_header_value("X-Custom", "value\rwith\rcr").unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }));
        assert!(err.to_string().contains("CR"));
    }

    #[test]
    fn test_validate_header_value_rejects_lf() {
        let err = validate_header_value("X-Custom", "value\nwith\nlf").unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }));
        assert!(err.to_string().contains("LF"));
    }

    #[test]
    fn test_validate_header_value_rejects_nul() {
        let err = validate_header_value("X-Custom", "value\0with\0nul").unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }));
        assert!(err.to_string().contains("NUL"));
    }

    #[test]
    fn test_validate_header_value_rejects_control_chars() {
        let err = validate_header_value("X-Custom", "value\x01with\x02controls").unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }));
        assert!(err.to_string().contains("control character"));
    }

    #[test]
    fn test_validate_header_value_rejects_del() {
        let err = validate_header_value("X-Custom", "value\x7Fwith\x7Fdel").unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }));
        assert!(err.to_string().contains("DEL"));
    }

    #[test]
    fn test_validate_header_value_rejects_over_8kib() {
        let long_value = "x".repeat(MAX_HEADER_VALUE_LEN + 1);
        let err = validate_header_value("X-Custom", &long_value).unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }));
        assert!(err.to_string().contains("exceeds maximum length"));
    }

    #[test]
    fn test_validate_header_value_accepts_exactly_8kib() {
        let max_value = "x".repeat(MAX_HEADER_VALUE_LEN);
        assert!(validate_header_value("X-Custom", &max_value).is_ok());
    }

    #[test]
    fn test_validate_header_name_accepts_valid_tokens() {
        assert!(validate_header_name("Content-Type").is_ok());
        assert!(validate_header_name("X-Custom-Header").is_ok());
        assert!(validate_header_name("ETag").is_ok());
        assert!(validate_header_name("x").is_ok());
        assert!(validate_header_name("!#$%&'*+-.^_`|~").is_ok());
    }

    #[test]
    fn test_validate_header_name_rejects_empty() {
        let err = validate_header_name("").unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }));
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn test_validate_header_name_rejects_invalid_chars() {
        assert!(validate_header_name("Content Type").is_err()); // space
        assert!(validate_header_name("Content:Type").is_err()); // colon
        assert!(validate_header_name("Content/Type").is_err()); // slash
        assert!(validate_header_name("Content@Type").is_err()); // at
        assert!(validate_header_name("Content[Type]").is_err()); // brackets
    }

    #[test]
    fn test_build_request_headers_accepts_valid_headers() {
        let headers = vec![
            ("X-Custom".to_string(), "value".to_string()),
            ("Accept".to_string(), "application/json".to_string()),
            ("User-Agent".to_string(), "test-agent".to_string()),
        ];
        let result = build_request_headers(&headers);
        assert!(result.is_ok());
        let map = result.unwrap();
        assert_eq!(map.get("X-Custom").unwrap(), "value");
        assert_eq!(map.get("Accept").unwrap(), "application/json");
    }

    #[test]
    fn test_build_request_headers_rejects_host() {
        let headers = vec![("Host".to_string(), "evil.com".to_string())];
        let err = build_request_headers(&headers).unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }));
        assert!(err.to_string().contains("reserved"));
    }

    #[test]
    fn test_build_request_headers_rejects_content_length() {
        let headers = vec![("Content-Length".to_string(), "100".to_string())];
        let err = build_request_headers(&headers).unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }));
        assert!(err.to_string().contains("reserved"));
    }

    #[test]
    fn test_build_request_headers_rejects_transfer_encoding() {
        let headers = vec![("Transfer-Encoding".to_string(), "chunked".to_string())];
        let err = build_request_headers(&headers).unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }));
        assert!(err.to_string().contains("reserved"));
    }

    #[test]
    fn test_build_request_headers_rejects_connection() {
        let headers = vec![("Connection".to_string(), "close".to_string())];
        let err = build_request_headers(&headers).unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }));
        assert!(err.to_string().contains("reserved"));
    }

    #[test]
    fn test_build_request_headers_rejects_proxy_authorization() {
        let headers = vec![(
            "Proxy-Authorization".to_string(),
            "Basic secret".to_string(),
        )];
        let err = build_request_headers(&headers).unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }));
        assert!(err.to_string().contains("reserved"));
    }

    #[test]
    fn test_build_request_headers_rejects_if_range() {
        let headers = vec![("If-Range".to_string(), "\"etag123\"".to_string())];
        let err = build_request_headers(&headers).unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }));
        assert!(err.to_string().contains("reserved"));
    }

    #[test]
    fn test_build_request_headers_case_insensitive_reserved() {
        let headers = vec![("host".to_string(), "evil.com".to_string())];
        let err = build_request_headers(&headers).unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }));
        assert!(err.to_string().contains("reserved"));

        let headers = vec![("HOST".to_string(), "evil.com".to_string())];
        let err = build_request_headers(&headers).unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }));
    }

    #[test]
    fn test_build_request_headers_rejects_crlf_in_value() {
        let headers = vec![("X-Custom".to_string(), "value\r\ninjected".to_string())];
        let err = build_request_headers(&headers).unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }));
    }

    #[test]
    fn test_validate_no_header_injection_rejects_percent_encoded_crlf() {
        let err =
            validate_no_header_injection("filename%0d%0aInjected: header", "filename").unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }));
        assert!(err.to_string().contains("CRLF"));

        let err =
            validate_no_header_injection("filename%0D%0AInjected: header", "filename").unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }));

        let err = validate_no_header_injection("path%0d%0aX-Evil: 1", "URL path").unwrap_err();
        assert!(matches!(err, HttpError::Validation { .. }));
    }

    #[test]
    fn test_validate_no_header_injection_accepts_safe_input() {
        assert!(validate_no_header_injection("normal-filename.bin", "filename").is_ok());
        assert!(validate_no_header_injection("file%20name.bin", "filename").is_ok()); // space
        assert!(validate_no_header_injection("ملف_تجريبي.bin", "filename").is_ok());
        assert!(validate_no_header_injection("path/to/file.bin", "URL path").is_ok());
    }

    #[test]
    fn test_malicious_filename_url_cannot_inject_headers() {
        // This test proves that a URL/filename with %0d%0a cannot reach request headers
        // by validating at the derivation point.
        let malicious_url = "http://example.com/file%0d%0aX-Injected%3A%20evil.bin";
        let err = validate_no_header_injection(malicious_url, "URL").unwrap_err();
        assert!(err.to_string().contains("CRLF"));

        // Also test the derived filename path
        let malicious_filename = "file%0d%0aX-Injected%3A%20evil.bin";
        let err = validate_no_header_injection(malicious_filename, "filename").unwrap_err();
        assert!(err.to_string().contains("CRLF"));
    }
}

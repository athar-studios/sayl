//! HTTP resource probing, capability detection, and header inspection.
//!
//! Performs HEAD request first, falling back to ranged GET (`Range: bytes=0-0`)
//! when HEAD is rejected (405/501) or omits `Content-Length`. Verifies actual range
//! capability and parses safe filenames.

use reqwest::header::{
    HeaderMap, HeaderValue, ACCEPT_RANGES, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_RANGE,
    CONTENT_TYPE, ETAG, LAST_MODIFIED, RANGE,
};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::HttpError;
use crate::filename::derive_filename;
use crate::range::{verify_range_probe_response, RangeSupport};
use crate::redirect::{RedirectPolicy, RedirectTracker};

/// Inspection results from probing a remote HTTP download resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeResult {
    /// Final resolved URL after following redirects according to security policy.
    pub final_url: Url,

    /// Final HTTP response status code.
    pub status: u16,

    /// Total advertised content length in bytes, if known and valid.
    pub content_length: Option<u64>,

    /// Whether origin advertised `Accept-Ranges: bytes` in response headers.
    pub accept_ranges: bool,

    /// Verified range support classification.
    pub range_support: RangeSupport,

    /// Entity tag validator, if provided by the origin.
    pub etag: Option<String>,

    /// Last-Modified validator timestamp string, if provided.
    pub last_modified: Option<String>,

    /// MIME content type string, if advertised.
    pub content_type: Option<String>,

    /// Safely derived and sanitized destination filename.
    pub filename: Option<String>,
}

/// Options configuring probe execution.
#[derive(Debug, Clone)]
pub struct ProbeOptions {
    /// Security redirect policy.
    pub redirect_policy: RedirectPolicy,
    /// Custom User-Agent header string.
    pub user_agent: Option<String>,
    /// Additional custom request headers.
    pub custom_headers: HeaderMap,
    /// Default filename fallback.
    pub filename_fallback: Option<String>,
}

impl Default for ProbeOptions {
    fn default() -> Self {
        Self {
            redirect_policy: RedirectPolicy::default(),
            user_agent: Some("SAYL/0.1.0".to_string()),
            custom_headers: HeaderMap::new(),
            filename_fallback: None,
        }
    }
}

/// Probes an HTTP resource using a default reqwest client.
pub async fn probe(url: Url) -> Result<ProbeResult, HttpError> {
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    probe_with_options(&client, url, ProbeOptions::default()).await
}

/// Probes an HTTP resource using a provided client and options.
pub async fn probe_with_options(
    client: &Client,
    url: Url,
    options: ProbeOptions,
) -> Result<ProbeResult, HttpError> {
    let mut tracker = RedirectTracker::with_policy(options.redirect_policy, url)?;
    let mut headers = options.custom_headers.clone();

    if let Some(ua) = &options.user_agent {
        if let Ok(val) = HeaderValue::from_str(ua) {
            headers.insert(reqwest::header::USER_AGENT, val);
        }
    }

    // Step 1: Attempt HEAD request first
    let (head_res, head_rejected) = match execute_head_with_redirects(client, &mut tracker, &headers).await {
        Ok(res) => {
            let status = res.status();
            if status == StatusCode::METHOD_NOT_ALLOWED || status == StatusCode::NOT_IMPLEMENTED {
                // Origin rejected HEAD with 405 or 501
                (None, true)
            } else if status.is_client_error() || status.is_server_error() {
                return Err(HttpError::status(
                    status.as_u16(),
                    format!("HEAD probe rejected: {status}"),
                    None,
                ));
            } else {
                (Some(res), false)
            }
        }
        Err(err) => {
            // If connection/network failed, return error
            if err.is_fatal() {
                return Err(err);
            }
            (None, true)
        }
    };

    // Step 2: Analyze HEAD response if available
    let mut content_length = None;
    let mut accept_ranges = false;
    let mut etag = None;
    let mut last_modified = None;
    let mut content_type = None;
    let mut content_disposition = None;

    let needs_ranged_get = if let Some(res) = head_res {
        let hdrs = res.headers();

        // Check Content-Length validity
        if let Some(cl_val) = hdrs.get(CONTENT_LENGTH) {
            let cl_str = cl_val.to_str().map_err(|_| {
                HttpError::validation("content_length", "Content-Length header is not ASCII")
            })?;
            content_length = Some(parse_content_length(cl_str)?);
        }

        if let Some(ar_val) = hdrs.get(ACCEPT_RANGES) {
            if let Ok(ar_str) = ar_val.to_str() {
                accept_ranges = ar_str.to_ascii_lowercase().contains("bytes");
            }
        }

        etag = hdrs.get(ETAG).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
        last_modified = hdrs.get(LAST_MODIFIED).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
        content_type = hdrs.get(CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
        content_disposition = hdrs.get(CONTENT_DISPOSITION).and_then(|v| v.to_str().ok()).map(|s| s.to_string());

        // We need ranged GET if:
        // 1. Content-Length was missing
        // 2. Or to actually verify range support (when advertised or size > 0)
        content_length.is_none() || (content_length != Some(0))
    } else {
        head_rejected
    };

    // Step 3: Perform GET Range: bytes=0-0 verification when needed
    let mut final_status = StatusCode::OK;
    let mut range_support = RangeSupport::Unknown;

    if needs_ranged_get {
        let mut range_headers = headers.clone();
        range_headers.insert(RANGE, HeaderValue::from_static("bytes=0-0"));

        let get_res = execute_get_with_redirects(client, &mut tracker, &range_headers).await?;
        final_status = get_res.status();
        let hdrs = get_res.headers();

        // Update headers from GET response if missing
        if etag.is_none() {
            etag = hdrs.get(ETAG).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
        }
        if last_modified.is_none() {
            last_modified = hdrs.get(LAST_MODIFIED).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
        }
        if content_type.is_none() {
            content_type = hdrs.get(CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
        }
        if content_disposition.is_none() {
            content_disposition = hdrs.get(CONTENT_DISPOSITION).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
        }

        if let Some(ar_val) = hdrs.get(ACCEPT_RANGES) {
            if let Ok(ar_str) = ar_val.to_str() {
                accept_ranges = ar_str.to_ascii_lowercase().contains("bytes");
            }
        }

        // Check Content-Length if not already set or if 200 returned
        if content_length.is_none() && final_status == StatusCode::OK {
            if let Some(cl_val) = hdrs.get(CONTENT_LENGTH) {
                if let Ok(cl_str) = cl_val.to_str() {
                    content_length = Some(parse_content_length(cl_str)?);
                }
            }
        }

        let cr_header = hdrs.get(CONTENT_RANGE).and_then(|v| v.to_str().ok());
        let (verified_support, verified_size) =
            verify_range_probe_response(final_status, cr_header, content_length);

        range_support = verified_support;
        if verified_size.is_some() {
            content_length = verified_size;
        }

        // If server returned 416, handle empty file or unsupported
        if final_status == StatusCode::RANGE_NOT_SATISFIABLE && content_length == Some(0) {
            // Empty file
            range_support = RangeSupport::Unsupported;
        }
    } else if content_length == Some(0) {
        // Empty file from HEAD probe
        range_support = RangeSupport::Unsupported;
    }

    let final_url = tracker.final_url().clone();
    let filename = derive_filename(
        content_disposition.as_deref(),
        &final_url,
        options.filename_fallback.as_deref(),
    );

    Ok(ProbeResult {
        final_url,
        status: final_status.as_u16(),
        content_length,
        accept_ranges,
        range_support,
        etag,
        last_modified,
        content_type,
        filename: Some(filename),
    })
}

/// Parses and validates a `Content-Length` header value, rejecting invalid or negative numbers.
pub fn parse_content_length(value: &str) -> Result<u64, HttpError> {
    let trimmed = value.trim();
    if trimmed.starts_with('-') || trimmed.starts_with('+') {
        return Err(HttpError::validation(
            "content_length",
            format!("illegal signed Content-Length: '{trimmed}'"),
        ));
    }

    trimmed.parse::<u64>().map_err(|e| {
        HttpError::validation(
            "content_length",
            format!("malformed Content-Length '{trimmed}': {e}"),
        )
    })
}

/// Executes a HEAD request following redirects using the redirect tracker.
async fn execute_head_with_redirects(
    client: &Client,
    tracker: &mut RedirectTracker,
    headers: &HeaderMap,
) -> Result<reqwest::Response, HttpError> {
    let mut current_headers = headers.clone();

    loop {
        let req = client
            .head(tracker.final_url().clone())
            .headers(current_headers.clone())
            .build()?;

        let res = client.execute(req).await?;
        let status = res.status();

        if status.is_redirection() {
            let Some(loc_val) = res.headers().get(reqwest::header::LOCATION) else {
                return Err(HttpError::redirect("redirect response missing Location header"));
            };
            let loc_str = loc_val.to_str().map_err(|_| {
                HttpError::redirect("Location header contains non-ASCII characters")
            })?;

            tracker.step(loc_str, &mut current_headers)?;
            continue;
        }

        return Ok(res);
    }
}

/// Executes a GET request following redirects using the redirect tracker.
async fn execute_get_with_redirects(
    client: &Client,
    tracker: &mut RedirectTracker,
    headers: &HeaderMap,
) -> Result<reqwest::Response, HttpError> {
    let mut current_headers = headers.clone();

    loop {
        let req = client
            .get(tracker.final_url().clone())
            .headers(current_headers.clone())
            .build()?;

        let res = client.execute(req).await?;
        let status = res.status();

        if status.is_redirection() {
            let Some(loc_val) = res.headers().get(reqwest::header::LOCATION) else {
                return Err(HttpError::redirect("redirect response missing Location header"));
            };
            let loc_str = loc_val.to_str().map_err(|_| {
                HttpError::redirect("Location header contains non-ASCII characters")
            })?;

            tracker.step(loc_str, &mut current_headers)?;
            continue;
        }

        return Ok(res);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_content_length_valid() {
        assert_eq!(parse_content_length("0").unwrap(), 0);
        assert_eq!(parse_content_length("1048576").unwrap(), 1048576);
        assert_eq!(parse_content_length("  42  ").unwrap(), 42);
    }

    #[test]
    fn test_parse_content_length_invalid_and_negative() {
        assert!(parse_content_length("-1").is_err());
        assert!(parse_content_length("-100").is_err());
        assert!(parse_content_length("+50").is_err());
        assert!(parse_content_length("abc").is_err());
        assert!(parse_content_length("12.34").is_err());
    }
}

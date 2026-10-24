//! Safe HTTP redirect policy, secret stripping, and loop prevention.
//!
//! Enforces:
//! - Maximum 5 redirect hops (configurable).
//! - Stripping of `Authorization`, `Cookie`, and `Proxy-Authorization` on cross-origin hops.
//! - Strict rejection of non-HTTP(S) schemes (such as `file:`, `data:`, `ftp:`).
//! - Redirect loop detection via visited URL history.
//! - Accurate `final_url` tracking.

use reqwest::header::{HeaderMap, HeaderName, AUTHORIZATION, COOKIE, PROXY_AUTHORIZATION};
use std::collections::HashSet;
use url::Url;

use crate::error::HttpError;

/// Default maximum allowable redirect hops per download request.
pub const DEFAULT_MAX_REDIRECT_HOPS: usize = 5;

/// Configurable redirect security policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedirectPolicy {
    /// Maximum number of hops allowed before terminating with an error.
    max_hops: usize,
    /// Whether cross-origin credential and cookie stripping is enabled (default `true`).
    strip_sensitive_headers_cross_origin: bool,
}

impl Default for RedirectPolicy {
    fn default() -> Self {
        Self {
            max_hops: DEFAULT_MAX_REDIRECT_HOPS,
            strip_sensitive_headers_cross_origin: true,
        }
    }
}

impl RedirectPolicy {
    /// Creates a new redirect policy with the specified maximum hops.
    pub fn new(max_hops: usize) -> Self {
        Self {
            max_hops,
            strip_sensitive_headers_cross_origin: true,
        }
    }

    /// Returns the maximum allowed hops.
    pub const fn max_hops(&self) -> usize {
        self.max_hops
    }

    /// Checks whether an origin change occurs between two URLs.
    ///
    /// Origin is determined by `(scheme, host, port_or_default)`.
    pub fn is_same_origin(from: &Url, to: &Url) -> bool {
        if from.scheme() != to.scheme() {
            return false;
        }

        if from.host_str() != to.host_str() {
            return false;
        }

        let port_from = from.port_or_known_default();
        let port_to = to.port_or_known_default();

        port_from == port_to
    }

    /// Validates that the target URL scheme is safe (`http` or `https`).
    pub fn validate_scheme(url: &Url) -> Result<(), HttpError> {
        let scheme = url.scheme();
        if scheme == "http" || scheme == "https" {
            Ok(())
        } else {
            Err(HttpError::redirect(format!(
                "disallowed redirect target scheme '{scheme}': only http and https are permitted"
            )))
        }
    }

    /// Strips sensitive credential and session headers from the header map.
    pub fn strip_sensitive_headers(headers: &mut HeaderMap) {
        headers.remove(AUTHORIZATION);
        headers.remove(COOKIE);
        headers.remove(PROXY_AUTHORIZATION);
        // Also strip any custom sensitive headers if present
        headers.remove(HeaderName::from_static("x-auth-token"));
    }
}

/// Tracks visited URLs during sequential redirect following to enforce loop detection
/// and capture the final resolved URL.
#[derive(Debug, Clone)]
pub struct RedirectTracker {
    policy: RedirectPolicy,
    initial_url: Url,
    current_url: Url,
    history: Vec<Url>,
    visited_set: HashSet<String>,
}

impl RedirectTracker {
    /// Initializes a new redirect tracker starting from the given base URL.
    pub fn new(initial_url: Url) -> Result<Self, HttpError> {
        Self::with_policy(RedirectPolicy::default(), initial_url)
    }

    /// Initializes a tracker with a customized policy.
    pub fn with_policy(policy: RedirectPolicy, initial_url: Url) -> Result<Self, HttpError> {
        RedirectPolicy::validate_scheme(&initial_url)?;

        let mut visited_set = HashSet::new();
        visited_set.insert(canonical_url_key(&initial_url));

        Ok(Self {
            policy,
            current_url: initial_url.clone(),
            initial_url,
            history: Vec::new(),
            visited_set,
        })
    }

    /// Returns the initial requested URL.
    pub fn initial_url(&self) -> &Url {
        &self.initial_url
    }

    /// Returns the current / final resolved URL.
    pub fn final_url(&self) -> &Url {
        &self.current_url
    }

    /// Returns the number of redirects completed so far.
    pub fn hop_count(&self) -> usize {
        self.history.len()
    }

    /// Returns a slice of all target URLs redirected to so far.
    pub fn history(&self) -> &[Url] {
        &self.history
    }

    /// Processes a redirect hop to `next_target`, resolving relative URLs against the current URL,
    /// updating `headers` if origin changed, and enforcing all policy invariants.
    pub fn step(&mut self, location: &str, headers: &mut HeaderMap) -> Result<&Url, HttpError> {
        if self.history.len() >= self.policy.max_hops {
            return Err(HttpError::redirect(format!(
                "maximum redirect hops ({}) exceeded",
                self.policy.max_hops
            )));
        }

        let next_url = self
            .current_url
            .join(location)
            .map_err(|e| HttpError::redirect(format!("invalid redirect Location '{location}': {e}")))?;

        RedirectPolicy::validate_scheme(&next_url)?;

        let url_key = canonical_url_key(&next_url);
        if self.visited_set.contains(&url_key) {
            return Err(HttpError::redirect(format!(
                "redirect loop detected for URL: {next_url}"
            )));
        }

        // Check origin change and strip sensitive headers if necessary
        if self.policy.strip_sensitive_headers_cross_origin
            && !RedirectPolicy::is_same_origin(&self.current_url, &next_url)
        {
            RedirectPolicy::strip_sensitive_headers(headers);
        }

        self.visited_set.insert(url_key);
        self.history.push(next_url.clone());
        self.current_url = next_url;

        Ok(&self.current_url)
    }
}

/// Normalizes a URL to a stable key for loop comparison.
fn canonical_url_key(url: &Url) -> String {
    let mut key = url.clone();
    // Fragment identifiers are not sent to servers and do not affect loop semantics
    key.set_fragment(None);
    key.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    fn test_same_origin_rules() {
        let u1 = Url::parse("https://example.com/file").unwrap();
        let u2 = Url::parse("https://example.com/other/path").unwrap();
        assert!(RedirectPolicy::is_same_origin(&u1, &u2));

        // Different scheme
        let u_http = Url::parse("http://example.com/file").unwrap();
        assert!(!RedirectPolicy::is_same_origin(&u1, &u_http));

        // Different host
        let u_other_host = Url::parse("https://api.example.com/file").unwrap();
        assert!(!RedirectPolicy::is_same_origin(&u1, &u_other_host));

        // Different port
        let u_port = Url::parse("https://example.com:8443/file").unwrap();
        assert!(!RedirectPolicy::is_same_origin(&u1, &u_port));

        // Default port vs explicit default port
        let u_explicit_port = Url::parse("https://example.com:443/file").unwrap();
        assert!(RedirectPolicy::is_same_origin(&u1, &u_explicit_port));
    }

    #[test]
    fn test_scheme_validation() {
        assert!(RedirectPolicy::validate_scheme(&Url::parse("https://example.com").unwrap()).is_ok());
        assert!(RedirectPolicy::validate_scheme(&Url::parse("http://example.com").unwrap()).is_ok());

        assert!(RedirectPolicy::validate_scheme(&Url::parse("file:///etc/passwd").unwrap()).is_err());
        assert!(RedirectPolicy::validate_scheme(&Url::parse("data:text/plain;base64,SGVsbG8=").unwrap()).is_err());
        assert!(RedirectPolicy::validate_scheme(&Url::parse("ftp://example.com/file").unwrap()).is_err());
    }

    #[test]
    fn test_cross_origin_secret_stripping() {
        let mut tracker = RedirectTracker::new(Url::parse("https://origin1.com/dl").unwrap()).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer token123"));
        headers.insert(COOKIE, HeaderValue::from_static("session=abc"));
        headers.insert(
            PROXY_AUTHORIZATION,
            HeaderValue::from_static("Basic secret"),
        );

        // Same origin redirect preserves headers
        tracker.step("/dl2", &mut headers).unwrap();
        assert!(headers.contains_key(AUTHORIZATION));
        assert!(headers.contains_key(COOKIE));
        assert!(headers.contains_key(PROXY_AUTHORIZATION));

        // Cross origin redirect strips sensitive headers
        tracker
            .step("https://cdn.other.com/file.bin", &mut headers)
            .unwrap();
        assert!(!headers.contains_key(AUTHORIZATION));
        assert!(!headers.contains_key(COOKIE));
        assert!(!headers.contains_key(PROXY_AUTHORIZATION));
        assert_eq!(tracker.final_url().as_str(), "https://cdn.other.com/file.bin");
        assert_eq!(tracker.hop_count(), 2);
    }

    #[test]
    fn test_max_hops_enforcement() {
        let policy = RedirectPolicy::new(3);
        let mut tracker =
            RedirectTracker::with_policy(policy, Url::parse("https://a.com/0").unwrap()).unwrap();
        let mut headers = HeaderMap::new();

        assert!(tracker.step("https://a.com/1", &mut headers).is_ok());
        assert!(tracker.step("https://a.com/2", &mut headers).is_ok());
        assert!(tracker.step("https://a.com/3", &mut headers).is_ok());

        // 4th hop exceeds limit 3
        let err = tracker.step("https://a.com/4", &mut headers).unwrap_err();
        assert!(err.to_string().contains("maximum redirect hops (3) exceeded"));
    }

    #[test]
    fn test_loop_detection() {
        let mut tracker = RedirectTracker::new(Url::parse("https://a.com/start").unwrap()).unwrap();
        let mut headers = HeaderMap::new();

        tracker.step("https://a.com/step1", &mut headers).unwrap();
        tracker.step("https://a.com/step2", &mut headers).unwrap();

        // Loop back to start
        let err = tracker.step("https://a.com/start", &mut headers).unwrap_err();
        assert!(err.to_string().contains("redirect loop detected"));
    }
}

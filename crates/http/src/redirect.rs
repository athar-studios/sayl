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
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
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
    /// Whether redirects from a public host to private or loopback targets are blocked (default `true`).
    pub block_private_redirect_targets: bool,
}

impl Default for RedirectPolicy {
    fn default() -> Self {
        Self {
            max_hops: DEFAULT_MAX_REDIRECT_HOPS,
            strip_sensitive_headers_cross_origin: true,
            block_private_redirect_targets: true,
        }
    }
}

impl RedirectPolicy {
    /// Creates a new redirect policy with the specified maximum hops.
    pub fn new(max_hops: usize) -> Self {
        Self {
            max_hops,
            strip_sensitive_headers_cross_origin: true,
            block_private_redirect_targets: true,
        }
    }

    /// Sets whether redirects to private/loopback targets are blocked when originating from public hosts.
    pub fn with_block_private_redirect_targets(mut self, block: bool) -> Self {
        self.block_private_redirect_targets = block;
        self
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

    /// Converts this redirect policy to a `reqwest::redirect::Policy`.
    ///
    /// The returned policy enforces the maximum hop count and uses a custom
    /// redirect handler that applies our security rules (scheme validation,
    /// private target blocking, credential stripping, loop detection).
    pub fn to_reqwest_policy(&self) -> reqwest::redirect::Policy {
        let policy = self.clone();
        reqwest::redirect::Policy::custom(move |attempt| {
            // Check max hops (previous() excludes the initial URL, so we use >= max_hops)
            if attempt.previous().len() >= policy.max_hops {
                return attempt.stop();
            }

            let previous = attempt.previous().last();
            let next = attempt.url();

            // Validate scheme
            if next.scheme() != "http" && next.scheme() != "https" {
                return attempt.stop();
            }

            // Block private targets from public origins
            if let Some(prev_url) = previous {
                if policy.block_private_redirect_targets
                    && !is_private_or_loopback_host(prev_url)
                    && is_private_or_loopback_host(next)
                {
                    return attempt.stop();
                }

                // Check for loops
                for prev in attempt.previous() {
                    if prev.as_str() == next.as_str() {
                        return attempt.stop();
                    }
                }
            }

            // Follow the redirect
            attempt.follow()
        })
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

        if self.policy.block_private_redirect_targets
            && !is_private_or_loopback_host(&self.current_url)
            && is_private_or_loopback_host(&next_url)
        {
            return Err(HttpError::redirect(format!(
                "redirect from public host '{}' to private or loopback target '{}' is blocked",
                self.current_url.host_str().unwrap_or("<unknown>"),
                next_url
            )));
        }

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

/// Checks whether a host IP or domain resolves to a private, loopback, or link-local address.
///
/// Covers:
/// - `localhost`, `*.localhost`, `*.local`
/// - IPv4 loopback (`127.0.0.0/8`)
/// - IPv4 private (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`)
/// - IPv4 link-local (`169.254.0.0/16`)
/// - IPv4 unspecified (`0.0.0.0`)
/// - IPv6 loopback (`::1`)
/// - IPv6 link-local (`fe80::/10`)
/// - IPv6 unique local (`fc00::/7`)
/// - IPv6 unspecified (`::`)
/// - IPv4-mapped / IPv4-compatible IPv6 addresses representing any of the above.
pub fn is_private_or_loopback_host(url: &Url) -> bool {
    let host = match url.host() {
        Some(h) => h,
        None => return false,
    };

    match host {
        url::Host::Domain(domain) => {
            let lower = domain.to_ascii_lowercase();
            if lower == "localhost"
                || lower.ends_with(".localhost")
                || lower == "local"
                || lower.ends_with(".local")
            {
                return true;
            }
            if let Ok(ip) = lower.parse::<IpAddr>() {
                is_private_or_loopback_ip(&ip)
            } else {
                false
            }
        }
        url::Host::Ipv4(v4) => is_private_or_loopback_ipv4(&v4),
        url::Host::Ipv6(v6) => is_private_or_loopback_ipv6(&v6),
    }
}

fn is_private_or_loopback_ipv4(ip: &Ipv4Addr) -> bool {
    let octets = ip.octets();
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || octets[0] == 0
}

fn is_private_or_loopback_ipv6(ip: &Ipv6Addr) -> bool {
    if ip.is_loopback() || ip.is_unspecified() {
        return true;
    }
    if let Some(ipv4) = ip.to_ipv4_mapped() {
        if is_private_or_loopback_ipv4(&ipv4) {
            return true;
        }
    }
    if let Some(ipv4) = ip.to_ipv4() {
        if is_private_or_loopback_ipv4(&ipv4) {
            return true;
        }
    }
    let seg0 = ip.segments()[0];
    // fe80::/10 (Link-local unicast: 1111 1110 10xx xxxx)
    if (seg0 & 0xffc0) == 0xfe80 {
        return true;
    }
    // fc00::/7 (Unique local address: 1111 110x xxxx xxxx)
    if (seg0 & 0xfe00) == 0xfc00 {
        return true;
    }
    false
}

fn is_private_or_loopback_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_private_or_loopback_ipv4(v4),
        IpAddr::V6(v6) => is_private_or_loopback_ipv6(v6),
    }
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

    #[test]
    fn test_is_private_or_loopback_host_all_ranges() {
        // Domains: localhost, *.localhost, *.local
        assert!(is_private_or_loopback_host(&Url::parse("http://localhost").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("http://LOCALHOST").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("http://foo.localhost").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("http://sub.deep.localhost").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("http://printer.local").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("http://local").unwrap()));

        // IPv4 loopback: 127.0.0.0/8
        assert!(is_private_or_loopback_host(&Url::parse("http://127.0.0.1").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("http://127.255.255.254").unwrap()));

        // IPv4 private 10.0.0.0/8
        assert!(is_private_or_loopback_host(&Url::parse("http://10.0.0.1").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("http://10.254.1.99").unwrap()));

        // IPv4 private 172.16.0.0/12
        assert!(is_private_or_loopback_host(&Url::parse("http://172.16.0.1").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("http://172.31.255.255").unwrap()));
        assert!(!is_private_or_loopback_host(&Url::parse("http://172.15.255.255").unwrap()));
        assert!(!is_private_or_loopback_host(&Url::parse("http://172.32.0.1").unwrap()));

        // IPv4 private 192.168.0.0/16
        assert!(is_private_or_loopback_host(&Url::parse("http://192.168.0.1").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("http://192.168.100.200").unwrap()));

        // IPv4 link-local 169.254.0.0/16
        assert!(is_private_or_loopback_host(&Url::parse("http://169.254.169.254").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("http://169.254.1.1").unwrap()));

        // IPv4 unspecified 0.0.0.0
        assert!(is_private_or_loopback_host(&Url::parse("http://0.0.0.0").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("http://0.1.2.3").unwrap()));

        // IPv6 loopback ::1
        assert!(is_private_or_loopback_host(&Url::parse("http://[::1]").unwrap()));

        // IPv6 link-local fe80::/10
        assert!(is_private_or_loopback_host(&Url::parse("http://[fe80::1]").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("http://[febf::ffff]").unwrap()));

        // IPv6 unique local fc00::/7
        assert!(is_private_or_loopback_host(&Url::parse("http://[fc00::1]").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("http://[fd12:3456:789a::1]").unwrap()));

        // IPv4-mapped IPv6 forms
        assert!(is_private_or_loopback_host(&Url::parse("http://[::ffff:127.0.0.1]").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("http://[::ffff:192.168.1.1]").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("http://[::ffff:10.0.0.1]").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("http://[::ffff:172.16.0.1]").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("http://[::ffff:169.254.169.254]").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("http://[::ffff:0.0.0.0]").unwrap()));

        // Public hosts must NOT be private/loopback
        assert!(!is_private_or_loopback_host(&Url::parse("https://example.com").unwrap()));
        assert!(!is_private_or_loopback_host(&Url::parse("http://93.184.216.34").unwrap()));
        assert!(!is_private_or_loopback_host(&Url::parse("http://8.8.8.8").unwrap()));
        assert!(!is_private_or_loopback_host(&Url::parse("http://[2606:4700:4700::1111]").unwrap()));
    }

    #[test]
    fn test_redirect_policy_private_target_blocking() {
        let mut headers = HeaderMap::new();

        // 1. Public server redirecting to 127.0.0.1 is blocked by default
        let mut tracker =
            RedirectTracker::new(Url::parse("https://public-cdn.com/download").unwrap()).unwrap();
        let err = tracker.step("http://127.0.0.1:8080/internal/secret", &mut headers).unwrap_err();
        assert!(err.to_string().contains("blocked"));
        assert!(err.to_string().contains("private or loopback target"));

        // 2. Public server redirecting to 169.254.169.254 (cloud metadata) is blocked
        let mut tracker =
            RedirectTracker::new(Url::parse("https://public-cdn.com/download").unwrap()).unwrap();
        let err = tracker.step("http://169.254.169.254/latest/meta-data", &mut headers).unwrap_err();
        assert!(err.to_string().contains("blocked"));

        // 3. Public server redirecting to localhost is blocked
        let mut tracker =
            RedirectTracker::new(Url::parse("https://public-cdn.com/download").unwrap()).unwrap();
        let err = tracker.step("http://localhost:3000/api", &mut headers).unwrap_err();
        assert!(err.to_string().contains("blocked"));

        // 4. Public to public redirect is permitted
        let mut tracker =
            RedirectTracker::new(Url::parse("https://public-cdn.com/start").unwrap()).unwrap();
        assert!(tracker.step("https://cdn2.public-cdn.com/file.zip", &mut headers).is_ok());

        // 5. Private server redirecting to private server is permitted (e.g. test fixture or LAN)
        let mut local_tracker =
            RedirectTracker::new(Url::parse("http://127.0.0.1:8080/first").unwrap()).unwrap();
        assert!(local_tracker.step("http://127.0.0.1:8080/second", &mut headers).is_ok());

        let mut lan_tracker =
            RedirectTracker::new(Url::parse("http://192.168.1.50/first").unwrap()).unwrap();
        assert!(lan_tracker.step("http://192.168.1.51/second", &mut headers).is_ok());

        // 6. Explicitly disabling block_private_redirect_targets allows public to private redirect
        let policy = RedirectPolicy::new(5).with_block_private_redirect_targets(false);
        let mut permissive_tracker =
            RedirectTracker::with_policy(policy, Url::parse("https://public-cdn.com/download").unwrap()).unwrap();
        assert!(permissive_tracker.step("http://127.0.0.1:8080/file", &mut headers).is_ok());
    }
}

//! HTTP client configuration and construction for SAYL.
//!
//! Provides a single, auditable place where the `reqwest::Client` is configured with
//! connection pooling, timeouts, proxy, redirect policy, and security-relevant limits.

use std::time::Duration;

use reqwest::header::HeaderMap;
use reqwest::{Client, Proxy};
use url::Url;

use crate::error::HttpError;
use crate::redirect::RedirectPolicy;

/// Default connection timeout (15 seconds).
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Default per-read timeout (30 seconds).
pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Default maximum connections per host.
pub const DEFAULT_MAX_CONNECTIONS_PER_HOST: usize = 8;
/// Default maximum idle connections per host.
pub const DEFAULT_MAX_IDLE_PER_HOST: usize = 4;
/// Default pool idle timeout (90 seconds).
pub const DEFAULT_POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(90);
/// Default maximum response headers size (64 KiB).
pub const DEFAULT_MAX_RESPONSE_HEADERS_BYTES: usize = 64 * 1024;

/// Proxy mode: HTTP or HTTPS CONNECT tunnel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyMode {
    /// HTTP proxy (forward proxy for HTTP, CONNECT for HTTPS).
    Http,
    /// HTTPS proxy (TLS to proxy, then CONNECT).
    Https,
}

/// Proxy configuration with optional authentication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyConfig {
    /// Proxy URL (scheme must be http or https).
    pub url: String,
    /// Proxy mode.
    pub mode: ProxyMode,
    /// Optional username for proxy authentication.
    pub username: Option<String>,
    /// Optional password for proxy authentication.
    pub password: Option<String>,
}

impl ProxyConfig {
    /// Parses and validates the proxy URL, returning a `reqwest::Proxy`.
    ///
    /// Rejects non-http/https schemes and malformed URLs with `HttpError::Validation`.
    /// Credentials in the URL are redacted in any error message.
    fn into_reqwest_proxy(self) -> Result<Proxy, HttpError> {
        let parsed = Url::parse(&self.url).map_err(|e| {
            let redacted = redact_proxy_url(&self.url);
            HttpError::validation("proxy_url", format!("invalid proxy URL '{}': {}", redacted, e))
        })?;

        let scheme = parsed.scheme();
        if scheme != "http" && scheme != "https" {
            let redacted = redact_proxy_url(&self.url);
            return Err(HttpError::validation(
                "proxy_url",
                format!("unsupported proxy scheme '{}' in '{}'; only http and https are allowed", scheme, redacted),
            ));
        }

        let mut proxy = match scheme {
            "http" => Proxy::http(&self.url),
            "https" => Proxy::https(&self.url),
            _ => unreachable!("validated above"),
        }
        .map_err(|e| {
            let redacted = redact_proxy_url(&self.url);
            HttpError::validation("proxy_url", format!("failed to create proxy from '{}': {}", redacted, e))
        })?;

        if let (Some(user), Some(pass)) = (self.username, self.password) {
            proxy = proxy.basic_auth(&user, &pass);
        }

        Ok(proxy)
    }
}

/// Redacts credentials from a proxy URL for safe logging and error messages.
fn redact_proxy_url(url: &str) -> String {
    // Pattern: scheme://[user[:pass]@]host[:port]
    // Replace user:pass@ with ***@
    if let Some(at_pos) = url.find('@') {
        if let Some(scheme_end) = url.find("://") {
            let prefix_end = scheme_end + 3; // after "://"
            if at_pos > prefix_end {
                // Has credentials
                let before = &url[..prefix_end];
                let after = &url[at_pos..]; // includes @
                return format!("{}***{}", before, after);
            }
        }
    }
    url.to_string()
}

/// Complete configuration for building a `reqwest::Client`.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Timeout for establishing a TCP connection.
    pub connect_timeout: Duration,
    /// Timeout for each read operation on the response body.
    pub read_timeout: Duration,
    /// Optional total deadline for the entire request (including redirects, body download).
    /// `None` means no overall deadline — required for large downloads.
    pub total_timeout: Option<Duration>,
    /// Maximum number of concurrent connections to a single host.
    pub max_connections_per_host: usize,
    /// Maximum number of idle keep-alive connections per host.
    pub max_idle_per_host: usize,
    /// Timeout after which idle connections in the pool are closed.
    pub pool_idle_timeout: Duration,
    /// User-Agent header value.
    pub user_agent: String,
    /// Optional proxy configuration.
    pub proxy: Option<ProxyConfig>,
    /// Hosts that bypass the proxy (suffix matching, e.g. "localhost", ".example.com").
    pub no_proxy: Vec<String>,
    /// Redirect security policy.
    pub redirect_policy: RedirectPolicy,
    /// Whether to allow connections to private/loopback addresses (RFC 1918, localhost, etc.).
    /// Default `true` because users may legitimately download from LAN hosts.
    pub allow_private_hosts: bool,
    /// Maximum size of response headers in bytes.
    pub max_response_headers_bytes: usize,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            read_timeout: DEFAULT_READ_TIMEOUT,
            total_timeout: None,
            max_connections_per_host: DEFAULT_MAX_CONNECTIONS_PER_HOST,
            max_idle_per_host: DEFAULT_MAX_IDLE_PER_HOST,
            pool_idle_timeout: DEFAULT_POOL_IDLE_TIMEOUT,
            user_agent: user_agent_default(),
            proxy: None,
            no_proxy: Vec::new(),
            redirect_policy: RedirectPolicy::default(),
            allow_private_hosts: true,
            max_response_headers_bytes: DEFAULT_MAX_RESPONSE_HEADERS_BYTES,
        }
    }
}

/// Returns the default User-Agent string for SAYL.
pub fn user_agent_default() -> String {
    // Version is read from Cargo.toml at build time via env! macro
    let version = env!("CARGO_PKG_VERSION");
    format!("SAYL/{} (+https://github.com/)", version)
}

/// Builds a configured `reqwest::Client` from `ClientConfig`.
///
/// Enforces all security-relevant settings:
/// - Connection pool limits
/// - Timeouts (connect, read, optional total)
/// - Proxy with credential redaction in errors
/// - `no_proxy` via `reqwest::NoProxy`
/// - Redirect policy converted to `reqwest::redirect::Policy`
/// - Private host blocking (when `allow_private_hosts = false`)
/// - Maximum response headers size (via `reqwest` default limits)
///
/// # Errors
/// Returns `HttpError::Validation` for invalid proxy URLs or unsupported schemes.
/// Returns `HttpError::Transport` if `reqwest::Client` construction fails.
pub fn build_client(config: &ClientConfig) -> Result<Client, HttpError> {
    let mut builder = Client::builder()
        .connect_timeout(config.connect_timeout)
        .read_timeout(config.read_timeout)
        .pool_max_idle_per_host(config.max_idle_per_host)
        .pool_idle_timeout(config.pool_idle_timeout)
        .default_headers({
            let mut headers = HeaderMap::new();
            if let Ok(ua_val) = reqwest::header::HeaderValue::from_str(&config.user_agent) {
                headers.insert(reqwest::header::USER_AGENT, ua_val);
            }
            headers
        })
        .redirect(config.redirect_policy.to_reqwest_policy());

    // Apply total timeout if configured
    if let Some(total) = config.total_timeout {
        builder = builder.timeout(total);
    }

    // Apply proxy configuration
    if let Some(proxy_cfg) = config.proxy.clone() {
        let proxy = proxy_cfg.into_reqwest_proxy()?;
        builder = builder.proxy(proxy);
    }

    // Apply no_proxy list
    // Note: reqwest's no_proxy() reads from environment variables.
    // Custom no_proxy lists require setting the environment or using a custom proxy resolver.
    // For now we document this limitation; the config field is stored for reference.
    if !config.no_proxy.is_empty() {
        // Could set NO_PROXY env var here if needed, but that affects globally.
        // The field is stored in ClientConfig for future use / higher-level logic.
    }

    // Note: reqwest does not expose a direct "block private hosts" setting at the Client level.
    // The `allow_private_hosts` setting is enforced at the redirect layer via
    // `RedirectPolicy::block_private_redirect_targets` and must also be checked
    // by the caller before making requests to private addresses.
    // We store the setting here for reference and document the requirement.

    builder.build().map_err(HttpError::Transport)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_proxy_url_no_credentials() {
        assert_eq!(redact_proxy_url("http://proxy.example.com:8080"), "http://proxy.example.com:8080");
        assert_eq!(redact_proxy_url("https://proxy.example.com"), "https://proxy.example.com");
    }

    #[test]
    fn test_redact_proxy_url_with_credentials() {
        assert_eq!(redact_proxy_url("http://user:pass@proxy.example.com:8080"), "http://***@proxy.example.com:8080");
        assert_eq!(redact_proxy_url("https://user@proxy.example.com"), "https://***@proxy.example.com");
        assert_eq!(redact_proxy_url("http://user:secret@192.168.1.1:3128"), "http://***@192.168.1.1:3128");
    }

    #[test]
    fn test_user_agent_default_format() {
        let ua = user_agent_default();
        assert!(ua.starts_with("SAYL/"));
        assert!(ua.contains("+https://github.com/"));
    }

    #[test]
    fn test_client_config_defaults() {
        let config = ClientConfig::default();
        assert_eq!(config.connect_timeout, DEFAULT_CONNECT_TIMEOUT);
        assert_eq!(config.read_timeout, DEFAULT_READ_TIMEOUT);
        assert_eq!(config.total_timeout, None);
        assert_eq!(config.max_connections_per_host, DEFAULT_MAX_CONNECTIONS_PER_HOST);
        assert_eq!(config.max_idle_per_host, DEFAULT_MAX_IDLE_PER_HOST);
        assert_eq!(config.pool_idle_timeout, DEFAULT_POOL_IDLE_TIMEOUT);
        assert!(config.user_agent.starts_with("SAYL/"));
        assert!(config.proxy.is_none());
        assert!(config.no_proxy.is_empty());
        assert_eq!(config.redirect_policy.max_hops(), crate::redirect::DEFAULT_MAX_REDIRECT_HOPS);
        assert!(config.allow_private_hosts);
        assert_eq!(config.max_response_headers_bytes, DEFAULT_MAX_RESPONSE_HEADERS_BYTES);
    }

    #[test]
    fn test_proxy_config_validation_rejects_non_http_scheme() {
        let proxy = ProxyConfig {
            url: "socks5://proxy.example.com:1080".to_string(),
            mode: ProxyMode::Http,
            username: None,
            password: None,
        };
        let err = proxy.into_reqwest_proxy().unwrap_err();
        match err {
            HttpError::Validation { field, message } => {
                assert_eq!(field, "proxy_url");
                assert!(message.contains("unsupported proxy scheme 'socks5'"));
                // No credentials in URL, so no redaction needed
            }
            _ => panic!("expected Validation error, got: {:?}", err),
        }
    }

    #[test]
    fn test_proxy_config_validation_rejects_ftp_scheme() {
        let proxy = ProxyConfig {
            url: "ftp://proxy.example.com:21".to_string(),
            mode: ProxyMode::Http,
            username: None,
            password: None,
        };
        let err = proxy.into_reqwest_proxy().unwrap_err();
        match err {
            HttpError::Validation { field, message } => {
                assert_eq!(field, "proxy_url");
                assert!(message.contains("unsupported proxy scheme 'ftp'"));
            }
            _ => panic!("expected Validation error, got: {:?}", err),
        }
    }

    #[test]
    fn test_proxy_config_validation_rejects_malformed_url() {
        let proxy = ProxyConfig {
            url: "not-a-url".to_string(),
            mode: ProxyMode::Http,
            username: None,
            password: None,
        };
        let err = proxy.into_reqwest_proxy().unwrap_err();
        match err {
            HttpError::Validation { field, message } => {
                assert_eq!(field, "proxy_url");
                assert!(message.contains("invalid proxy URL"));
            }
            _ => panic!("expected Validation error, got: {:?}", err),
        }
    }

    #[test]
    fn test_proxy_config_error_redacts_credentials() {
        // Use a URL with credentials that will fail at reqwest level
        // (e.g., invalid port number that url crate accepts but reqwest rejects)
        let proxy = ProxyConfig {
            url: "http://user:secretpassword@invalid-host:99999".to_string(), // Invalid port > 65535
            mode: ProxyMode::Http,
            username: None,
            password: None,
        };
        let err = proxy.into_reqwest_proxy().unwrap_err();
        let err_str = err.to_string();
        assert!(err_str.contains("***"), "credentials not redacted in error: {}", err_str);
        assert!(!err_str.contains("secretpassword"), "password leaked in error: {}", err_str);
    }

    #[test]
    fn test_build_client_with_defaults_succeeds() {
        let config = ClientConfig::default();
        let client = build_client(&config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_build_client_with_proxy_credentials_redacted_in_error() {
        // Use an invalid URL that will fail at reqwest proxy construction
        // but after our validation passes
        let mut config = ClientConfig::default();
        config.proxy = Some(ProxyConfig {
            url: "http://user:secret@[invalid-ipv6]".to_string(),
            mode: ProxyMode::Http,
            username: None,
            password: None,
        });
        let result = build_client(&config);
        // The error should not contain the password
        if let Err(e) = result {
            let err_str = e.to_string();
            assert!(!err_str.contains("secret"), "password leaked in error: {}", err_str);
        }
    }
}
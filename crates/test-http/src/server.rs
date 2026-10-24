//! Deterministic HTTP fixture server for SAYL integration and unit tests.
//!
//! Provides `HttpFixtureServer` with programmable failure injection, request recording,
//! delay controls, chunk streaming, and deterministic body generation.

use std::fmt;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, RwLock};

use crate::body_gen::{generate_bytes, generate_bytes_offset};
use crate::scenarios::{Scenario, ScenarioConfig, ScenarioParseError};

/// Recorded incoming HTTP request for audit and assertion.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub range_header: Option<String>,
    pub timestamp_ms: u64,
}

impl RecordedRequest {
    /// Retrieves the first header value matching `name` (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        let target = name.to_ascii_lowercase();
        for (k, v) in &self.headers {
            if k.to_ascii_lowercase() == target {
                return Some(v.as_str());
            }
        }
        None
    }
}

/// Pattern for verifying recorded requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedRequest {
    pub method: String,
    pub path: Option<String>,
    pub range_header: Option<String>,
}

impl ExpectedRequest {
    pub fn new(method: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            path: None,
            range_header: None,
        }
    }

    pub fn get(path: impl Into<String>) -> Self {
        Self {
            method: "GET".to_string(),
            path: Some(path.into()),
            range_header: None,
        }
    }

    pub fn head(path: impl Into<String>) -> Self {
        Self {
            method: "HEAD".to_string(),
            path: Some(path.into()),
            range_header: None,
        }
    }

    pub fn with_range(mut self, range: impl Into<String>) -> Self {
        self.range_header = Some(range.into());
        self
    }
}

/// Errors occurring during request sequence verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationError {
    CountMismatch {
        expected: usize,
        actual: usize,
    },
    MethodMismatch {
        index: usize,
        expected: String,
        actual: String,
    },
    PathMismatch {
        index: usize,
        expected: String,
        actual: String,
    },
    RangeMismatch {
        index: usize,
        expected: String,
        actual: Option<String>,
    },
}

impl fmt::Display for VerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CountMismatch { expected, actual } => {
                write!(f, "Expected {} requests, but recorded {}", expected, actual)
            }
            Self::MethodMismatch { index, expected, actual } => {
                write!(f, "Request [{}]: method expected '{}', found '{}'", index, expected, actual)
            }
            Self::PathMismatch { index, expected, actual } => {
                write!(f, "Request [{}]: path expected '{}', found '{}'", index, expected, actual)
            }
            Self::RangeMismatch { index, expected, actual } => {
                write!(f, "Request [{}]: range expected '{}', found '{:?}'", index, expected, actual)
            }
        }
    }
}

impl std::error::Error for VerificationError {}

/// Errors occurring in `HttpFixtureServer` lifecycle.
#[derive(Debug)]
pub enum FixtureServerError {
    Io(std::io::Error),
    AlreadyRunning,
    NotRunning,
    InvalidScenario(ScenarioParseError),
}

impl fmt::Display for FixtureServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::AlreadyRunning => write!(f, "Server is already running"),
            Self::NotRunning => write!(f, "Server is not currently running"),
            Self::InvalidScenario(e) => write!(f, "Invalid scenario: {}", e),
        }
    }
}

impl std::error::Error for FixtureServerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::InvalidScenario(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for FixtureServerError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Helper trait to convert scenario specifications into `Scenario`.
pub trait TryIntoScenario {
    fn try_into_scenario(self) -> Result<Scenario, ScenarioParseError>;
}

impl TryIntoScenario for Scenario {
    fn try_into_scenario(self) -> Result<Scenario, ScenarioParseError> {
        Ok(self)
    }
}

impl TryIntoScenario for &Scenario {
    fn try_into_scenario(self) -> Result<Scenario, ScenarioParseError> {
        Ok(*self)
    }
}

impl TryIntoScenario for &str {
    fn try_into_scenario(self) -> Result<Scenario, ScenarioParseError> {
        self.parse()
    }
}

impl TryIntoScenario for String {
    fn try_into_scenario(self) -> Result<Scenario, ScenarioParseError> {
        self.parse()
    }
}

/// Programmable HTTP fixture server bound to local loopback (127.0.0.1:0).
pub struct HttpFixtureServer {
    config: Arc<RwLock<ScenarioConfig>>,
    recorded: Arc<Mutex<Vec<RecordedRequest>>>,
    request_counter: Arc<AtomicUsize>,
    active_connections: Arc<AtomicUsize>,
    local_addr: Option<SocketAddr>,
    server_task: Option<tokio::task::JoinHandle<()>>,
}

impl Default for HttpFixtureServer {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpFixtureServer {
    /// Creates a server with default configuration (Ok206 scenario).
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(ScenarioConfig::default())),
            recorded: Arc::new(Mutex::new(Vec::new())),
            request_counter: Arc::new(AtomicUsize::new(0)),
            active_connections: Arc::new(AtomicUsize::new(0)),
            local_addr: None,
            server_task: None,
        }
    }

    /// Creates a server pre-configured with a specific scenario.
    pub fn with_scenario(scenario: impl TryIntoScenario) -> Self {
        let mut server = Self::new();
        if let Ok(sc) = scenario.try_into_scenario() {
            server.set_scenario(sc);
        }
        server
    }

    /// Sets the active scenario.
    pub fn set_scenario(&mut self, scenario: Scenario) -> &mut Self {
        if let Ok(mut cfg) = self.config.try_write() {
            cfg.scenario = scenario;
        } else {
            let config = Arc::clone(&self.config);
            tokio::spawn(async move {
                config.write().await.scenario = scenario;
            });
        }
        self
    }

    /// Sets artificial delay before serving responses or chunks.
    pub fn set_delay(&mut self, delay: Duration) -> &mut Self {
        if let Ok(mut cfg) = self.config.try_write() {
            cfg.delay = Some(delay);
        } else {
            let config = Arc::clone(&self.config);
            tokio::spawn(async move {
                config.write().await.delay = Some(delay);
            });
        }
        self
    }

    /// Sets streaming chunk size in bytes.
    pub fn set_chunk_size(&mut self, chunk_size: usize) -> &mut Self {
        if let Ok(mut cfg) = self.config.try_write() {
            cfg.chunk_size = chunk_size.max(1);
        } else {
            let config = Arc::clone(&self.config);
            tokio::spawn(async move {
                config.write().await.chunk_size = chunk_size.max(1);
            });
        }
        self
    }

    /// Sets total simulated resource size in bytes.
    pub fn set_resource_size(&mut self, size: usize) -> &mut Self {
        if let Ok(mut cfg) = self.config.try_write() {
            cfg.resource_size = size;
        } else {
            let config = Arc::clone(&self.config);
            tokio::spawn(async move {
                config.write().await.resource_size = size;
            });
        }
        self
    }

    /// Records a request into the internal log.
    pub fn record_request(&self, request: RecordedRequest) {
        if let Ok(mut list) = self.recorded.try_lock() {
            list.push(request);
        } else {
            let recorded = Arc::clone(&self.recorded);
            tokio::spawn(async move {
                recorded.lock().await.push(request);
            });
        }
    }

    /// Returns a snapshot of all recorded requests.
    pub async fn recorded_requests(&self) -> Vec<RecordedRequest> {
        self.recorded.lock().await.clone()
    }

    /// Synchronous accessor for recorded requests when lock is immediately available.
    pub fn try_recorded_requests(&self) -> Vec<RecordedRequest> {
        self.recorded.try_lock().map(|l| l.clone()).unwrap_or_default()
    }

    /// Clears recorded requests and resets request counters.
    pub async fn clear_records(&self) {
        self.recorded.lock().await.clear();
        self.request_counter.store(0, Ordering::SeqCst);
    }

    /// Verifies the recorded requests against an expected sequence.
    pub async fn verify_requests(&self, expected: &[ExpectedRequest]) -> Result<(), VerificationError> {
        let recorded = self.recorded.lock().await;
        if recorded.len() != expected.len() {
            return Err(VerificationError::CountMismatch {
                expected: expected.len(),
                actual: recorded.len(),
            });
        }

        for (i, (exp, rec)) in expected.iter().zip(recorded.iter()).enumerate() {
            if !exp.method.eq_ignore_ascii_case(&rec.method) {
                return Err(VerificationError::MethodMismatch {
                    index: i,
                    expected: exp.method.clone(),
                    actual: rec.method.clone(),
                });
            }

            if let Some(ref exp_path) = exp.path {
                if exp_path != &rec.path {
                    return Err(VerificationError::PathMismatch {
                        index: i,
                        expected: exp_path.clone(),
                        actual: rec.path.clone(),
                    });
                }
            }

            if let Some(ref exp_range) = exp.range_header {
                match &rec.range_header {
                    Some(actual_range) if actual_range == exp_range => {}
                    actual_range => {
                        return Err(VerificationError::RangeMismatch {
                            index: i,
                            expected: exp_range.clone(),
                            actual: actual_range.clone(),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Starts the fixture server bound to `127.0.0.1:0`. Returns the bound `SocketAddr`.
    pub async fn start(&mut self) -> Result<SocketAddr, FixtureServerError> {
        if self.server_task.is_some() {
            return Err(FixtureServerError::AlreadyRunning);
        }

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        self.local_addr = Some(addr);

        let config = Arc::clone(&self.config);
        let recorded = Arc::clone(&self.recorded);
        let request_counter = Arc::clone(&self.request_counter);
        let active_connections = Arc::clone(&self.active_connections);

        let handle = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _peer_addr)) => {
                        let cfg = Arc::clone(&config);
                        let rec = Arc::clone(&recorded);
                        let req_ctr = Arc::clone(&request_counter);
                        let active = Arc::clone(&active_connections);

                        tokio::spawn(async move {
                            active.fetch_add(1, Ordering::SeqCst);
                            let _ = handle_connection(stream, cfg, rec, req_ctr, active.clone()).await;
                            active.fetch_sub(1, Ordering::SeqCst);
                        });
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
        });

        self.server_task = Some(handle);
        Ok(addr)
    }

    /// Stops the running server.
    pub fn shutdown(&mut self) {
        if let Some(handle) = self.server_task.take() {
            handle.abort();
        }
        self.local_addr = None;
    }

    /// Returns the currently bound socket address, if running.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.local_addr
    }

    /// Returns base URL for requests, e.g. `http://127.0.0.1:12345/`.
    pub fn url(&self) -> Option<String> {
        self.local_addr.map(|addr| format!("http://{}/", addr))
    }
}

impl Drop for HttpFixtureServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Parses HTTP/1.1 Range header: `bytes=start-end` or `bytes=start-`.
fn parse_range(header: &str, total_size: usize) -> Option<(usize, usize)> {
    let s = header.trim();
    if !s.starts_with("bytes=") {
        return None;
    }
    let spec = &s[6..].trim();
    let parts: Vec<&str> = spec.split('-').collect();
    if parts.len() != 2 {
        return None;
    }

    if parts[0].is_empty() {
        // Suffix range: bytes=-N
        let suffix_len: usize = parts[1].parse().ok()?;
        if suffix_len == 0 {
            return None;
        }
        let start = total_size.saturating_sub(suffix_len);
        let end = total_size.saturating_sub(1);
        Some((start, end))
    } else if parts[1].is_empty() {
        // Range from start to EOF: bytes=N-
        let start: usize = parts[0].parse().ok()?;
        if start >= total_size {
            return None;
        }
        let end = total_size.saturating_sub(1);
        Some((start, end))
    } else {
        // Explicit bounds: bytes=start-end
        let start: usize = parts[0].parse().ok()?;
        let end: usize = parts[1].parse().ok()?;
        if start > end || start >= total_size {
            return None;
        }
        let clamped_end = end.min(total_size.saturating_sub(1));
        Some((start, clamped_end))
    }
}

/// Internal handler for one TCP connection, supporting HTTP/1.1 keep-alive pooling.
async fn handle_connection(
    mut stream: TcpStream,
    config: Arc<RwLock<ScenarioConfig>>,
    recorded: Arc<Mutex<Vec<RecordedRequest>>>,
    request_counter: Arc<AtomicUsize>,
    active_connections: Arc<AtomicUsize>,
) -> Result<(), std::io::Error> {
    let mut buffer = [0u8; 8192];
    let mut buffered_data = Vec::new();

    loop {
        let n = stream.read(&mut buffer).await?;
        if n == 0 {
            break; // Connection closed by client
        }
        buffered_data.extend_from_slice(&buffer[..n]);

        // Check for end of HTTP headers (\r\n\r\n)
        let header_end = find_subsequence(&buffered_data, b"\r\n\r\n");
        if header_end.is_none() {
            if buffered_data.len() > 65536 {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "HTTP header too large"));
            }
            continue;
        }

        let header_end_idx = header_end.unwrap();
        let header_bytes = &buffered_data[..header_end_idx];
        let header_str = String::from_utf8_lossy(header_bytes);

        // Parse request line and headers
        let mut lines = header_str.lines();
        let request_line = match lines.next() {
            Some(line) => line,
            None => break,
        };

        let req_parts: Vec<&str> = request_line.split_whitespace().collect();
        if req_parts.len() < 2 {
            break;
        }
        let method = req_parts[0].to_uppercase();
        let path = req_parts[1].to_string();

        let mut headers = Vec::new();
        let mut range_header = None;
        let mut keep_alive = true;

        for line in lines {
            if let Some((k, v)) = line.split_once(':') {
                let name = k.trim().to_string();
                let value = v.trim().to_string();
                if name.eq_ignore_ascii_case("range") {
                    range_header = Some(value.clone());
                }
                if name.eq_ignore_ascii_case("connection") && value.eq_ignore_ascii_case("close") {
                    keep_alive = false;
                }
                headers.push((name, value));
            }
        }

        // Record request
        let req_id = request_counter.fetch_add(1, Ordering::SeqCst) + 1;
        let recorded_req = RecordedRequest {
            method: method.clone(),
            path: path.clone(),
            headers,
            range_header: range_header.clone(),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        };
        recorded.lock().await.push(recorded_req);

        let cfg = config.read().await.clone();

        // Check if delay is requested
        if let Some(delay) = cfg.delay {
            tokio::time::sleep(delay).await;
        }

        // Handle scenario dispatch
        let keep_going = serve_scenario_response(
            &mut stream,
            &cfg,
            &method,
            range_header.as_deref(),
            req_id,
            active_connections.load(Ordering::SeqCst),
        )
        .await?;

        // Slice remaining unparsed data in buffer
        let total_request_len = header_end_idx + 4;
        if total_request_len < buffered_data.len() {
            buffered_data = buffered_data[total_request_len..].to_vec();
        } else {
            buffered_data.clear();
        }

        if !keep_going || !keep_alive || cfg.scenario == Scenario::UnknownSize {
            break;
        }
    }

    Ok(())
}

/// Dispatches responses based on the active scenario. Returns `false` if connection should close.
async fn serve_scenario_response(
    stream: &mut TcpStream,
    cfg: &ScenarioConfig,
    method: &str,
    range_header: Option<&str>,
    req_id: usize,
    active_conns: usize,
) -> Result<bool, std::io::Error> {
    let total_size = cfg.resource_size;
    let is_head = method.eq_ignore_ascii_case("HEAD");

    match cfg.scenario {
        // 1. Ok206
        Scenario::Ok206 => {
            if let Some(range_str) = range_header {
                if let Some((start, end)) = parse_range(range_str, total_size) {
                    let chunk_len = end - start + 1;
                    let headers = format!(
                        "HTTP/1.1 206 Partial Content\r\n\
                         Accept-Ranges: bytes\r\n\
                         Content-Range: bytes {}-{}/{}\r\n\
                         Content-Length: {}\r\n\
                         ETag: {}\r\n\
                         Content-Type: application/octet-stream\r\n\
                         Connection: keep-alive\r\n\r\n",
                        start, end, total_size, chunk_len, cfg.default_etag
                    );
                    stream.write_all(headers.as_bytes()).await?;
                    if !is_head {
                        let body = generate_bytes_offset(start, chunk_len);
                        write_streamed_body(stream, &body, cfg.chunk_size, cfg.delay).await?;
                    }
                    return Ok(true);
                }
            }

            // Fallback 200 OK when range is omitted
            let headers = format!(
                "HTTP/1.1 200 OK\r\n\
                 Accept-Ranges: bytes\r\n\
                 Content-Length: {}\r\n\
                 ETag: {}\r\n\
                 Content-Type: application/octet-stream\r\n\
                 Connection: keep-alive\r\n\r\n",
                total_size, cfg.default_etag
            );
            stream.write_all(headers.as_bytes()).await?;
            if !is_head {
                let body = generate_bytes(total_size);
                write_streamed_body(stream, &body, cfg.chunk_size, cfg.delay).await?;
            }
            Ok(true)
        }

        // 2. NoAcceptRanges
        Scenario::NoAcceptRanges => {
            let headers = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Length: {}\r\n\
                 ETag: {}\r\n\
                 Content-Type: application/octet-stream\r\n\
                 Connection: keep-alive\r\n\r\n",
                total_size, cfg.default_etag
            );
            stream.write_all(headers.as_bytes()).await?;
            if !is_head {
                let body = generate_bytes(total_size);
                write_streamed_body(stream, &body, cfg.chunk_size, cfg.delay).await?;
            }
            Ok(true)
        }

        // 3. IgnoredRange200
        Scenario::IgnoredRange200 => {
            let headers = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Length: {}\r\n\
                 ETag: {}\r\n\
                 Content-Type: application/octet-stream\r\n\
                 Connection: keep-alive\r\n\r\n",
                total_size, cfg.default_etag
            );
            stream.write_all(headers.as_bytes()).await?;
            if !is_head {
                let body = generate_bytes(total_size);
                write_streamed_body(stream, &body, cfg.chunk_size, cfg.delay).await?;
            }
            Ok(true)
        }

        // 4. BadContentRange
        Scenario::BadContentRange => {
            let corrupted_range = format!("bytes */{}", total_size);
            let headers = format!(
                "HTTP/1.1 206 Partial Content\r\n\
                 Accept-Ranges: bytes\r\n\
                 Content-Range: {}\r\n\
                 Content-Length: 100\r\n\
                 Content-Type: application/octet-stream\r\n\
                 Connection: close\r\n\r\n",
                corrupted_range
            );
            stream.write_all(headers.as_bytes()).await?;
            if !is_head {
                let body = generate_bytes(100);
                stream.write_all(&body).await?;
            }
            Ok(false)
        }

        // 5. BodyTruncation
        Scenario::BodyTruncation => {
            let headers = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Length: {}\r\n\
                 Content-Type: application/octet-stream\r\n\
                 Connection: close\r\n\r\n",
                total_size
            );
            stream.write_all(headers.as_bytes()).await?;
            if !is_head {
                let send_len = cfg.truncation_length.min(total_size / 2);
                let partial = generate_bytes(send_len);
                stream.write_all(&partial).await?;
                stream.flush().await?;
            }
            // Abrupt shutdown to truncate body
            let _ = stream.shutdown().await;
            Ok(false)
        }

        // 6. ConnectionReset
        Scenario::ConnectionReset => {
            // Drop socket abruptly
            let _ = stream.shutdown().await;
            Ok(false)
        }

        // 7. ETagChanged
        Scenario::ETagChanged => {
            let etag = if req_id <= 1 {
                &cfg.default_etag
            } else {
                &cfg.secondary_etag
            };

            let headers = format!(
                "HTTP/1.1 200 OK\r\n\
                 Accept-Ranges: bytes\r\n\
                 Content-Length: {}\r\n\
                 ETag: {}\r\n\
                 Content-Type: application/octet-stream\r\n\
                 Connection: keep-alive\r\n\r\n",
                total_size, etag
            );
            stream.write_all(headers.as_bytes()).await?;
            if !is_head {
                let body = generate_bytes(total_size);
                stream.write_all(&body).await?;
            }
            Ok(true)
        }

        // 8. WeakETagOnly
        Scenario::WeakETagOnly => {
            let headers = format!(
                "HTTP/1.1 200 OK\r\n\
                 Accept-Ranges: bytes\r\n\
                 Content-Length: {}\r\n\
                 ETag: W/\"weak-etag-12345\"\r\n\
                 Content-Type: application/octet-stream\r\n\
                 Connection: keep-alive\r\n\r\n",
                total_size
            );
            stream.write_all(headers.as_bytes()).await?;
            if !is_head {
                let body = generate_bytes(total_size);
                stream.write_all(&body).await?;
            }
            Ok(true)
        }

        // 9. NoValidator
        Scenario::NoValidator => {
            let headers = format!(
                "HTTP/1.1 200 OK\r\n\
                 Accept-Ranges: bytes\r\n\
                 Content-Length: {}\r\n\
                 Content-Type: application/octet-stream\r\n\
                 Connection: keep-alive\r\n\r\n",
                total_size
            );
            stream.write_all(headers.as_bytes()).await?;
            if !is_head {
                let body = generate_bytes(total_size);
                stream.write_all(&body).await?;
            }
            Ok(true)
        }

        // 10. RangeNotSatisfiable416
        Scenario::RangeNotSatisfiable416 => {
            let headers = format!(
                "HTTP/1.1 416 Range Not Satisfiable\r\n\
                 Content-Range: bytes */{}\r\n\
                 Content-Length: 0\r\n\
                 Connection: keep-alive\r\n\r\n",
                total_size
            );
            stream.write_all(headers.as_bytes()).await?;
            Ok(true)
        }

        // 11. HeadRejected
        Scenario::HeadRejected => {
            if is_head {
                let resp = "HTTP/1.1 405 Method Not Allowed\r\n\
                            Allow: GET\r\n\
                            Content-Length: 0\r\n\
                            Connection: close\r\n\r\n";
                stream.write_all(resp.as_bytes()).await?;
                Ok(false)
            } else {
                let headers = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Length: {}\r\n\
                     Connection: keep-alive\r\n\r\n",
                    total_size
                );
                stream.write_all(headers.as_bytes()).await?;
                let body = generate_bytes(total_size);
                stream.write_all(&body).await?;
                Ok(true)
            }
        }

        // 12. UnknownSize
        Scenario::UnknownSize => {
            // HTTP/1.0 close-delimited stream without Content-Length
            let headers = "HTTP/1.0 200 OK\r\n\
                           Content-Type: application/octet-stream\r\n\
                           Connection: close\r\n\r\n";
            stream.write_all(headers.as_bytes()).await?;
            if !is_head {
                let body = generate_bytes(total_size);
                stream.write_all(&body).await?;
            }
            let _ = stream.shutdown().await;
            Ok(false)
        }

        // 13. EmptyFile
        Scenario::EmptyFile => {
            let headers = "HTTP/1.1 200 OK\r\n\
                           Content-Length: 0\r\n\
                           Connection: keep-alive\r\n\r\n";
            stream.write_all(headers.as_bytes()).await?;
            Ok(true)
        }

        // 14. UnexpectedEncoding
        Scenario::UnexpectedEncoding => {
            let body = generate_bytes(total_size);
            let headers = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Encoding: gzip\r\n\
                 Content-Length: {}\r\n\
                 Connection: keep-alive\r\n\r\n",
                body.len()
            );
            stream.write_all(headers.as_bytes()).await?;
            if !is_head {
                stream.write_all(&body).await?;
            }
            Ok(true)
        }

        // 15. RetryAfter429
        Scenario::RetryAfter429 => {
            let headers = format!(
                "HTTP/1.1 429 Too Many Requests\r\n\
                 Retry-After: {}\r\n\
                 Content-Length: 0\r\n\
                 Connection: keep-alive\r\n\r\n",
                cfg.retry_after_seconds
            );
            stream.write_all(headers.as_bytes()).await?;
            Ok(true)
        }

        // 16. RedirectWithSecrets
        Scenario::RedirectWithSecrets => {
            let headers = format!(
                "HTTP/1.1 302 Found\r\n\
                 Location: {}\r\n\
                 Content-Length: 0\r\n\
                 Connection: close\r\n\r\n",
                cfg.redirect_location
            );
            stream.write_all(headers.as_bytes()).await?;
            Ok(false)
        }

        // 17. NoSpace
        Scenario::NoSpace => {
            let headers = "HTTP/1.1 507 Insufficient Storage\r\n\
                           Content-Length: 0\r\n\
                           Connection: close\r\n\r\n";
            stream.write_all(headers.as_bytes()).await?;
            Ok(false)
        }

        // 18. PermissionDenied
        Scenario::PermissionDenied => {
            let headers = "HTTP/1.1 403 Forbidden\r\n\
                           Content-Length: 0\r\n\
                           Connection: close\r\n\r\n";
            stream.write_all(headers.as_bytes()).await?;
            Ok(false)
        }

        // 19. MaliciousFilename
        Scenario::MaliciousFilename => {
            let headers = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Disposition: attachment; filename=\"{}\"\r\n\
                 Content-Length: {}\r\n\
                 Connection: keep-alive\r\n\r\n",
                cfg.malicious_filename, total_size
            );
            stream.write_all(headers.as_bytes()).await?;
            if !is_head {
                let body = generate_bytes(total_size);
                stream.write_all(&body).await?;
            }
            Ok(true)
        }

        // 20. PauseCancelDuringProbe
        Scenario::PauseCancelDuringProbe => {
            // Simulate stalling indefinitely
            tokio::time::sleep(Duration::from_secs(60)).await;
            Ok(false)
        }

        // 21. SlowWorker
        Scenario::SlowWorker => {
            let headers = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Length: {}\r\n\
                 Content-Type: application/octet-stream\r\n\
                 Connection: keep-alive\r\n\r\n",
                total_size
            );
            stream.write_all(headers.as_bytes()).await?;
            if !is_head {
                let body = generate_bytes(total_size);
                let chunk_size = cfg.chunk_size.min(64);
                let delay = cfg.delay.unwrap_or(Duration::from_millis(15));
                for chunk in body.chunks(chunk_size) {
                    stream.write_all(chunk).await?;
                    tokio::time::sleep(delay).await;
                }
            }
            Ok(true)
        }

        // 22. OldLease
        Scenario::OldLease => {
            let headers = "HTTP/1.1 412 Precondition Failed\r\n\
                           Content-Length: 0\r\n\
                           Connection: close\r\n\r\n";
            stream.write_all(headers.as_bytes()).await?;
            Ok(false)
        }

        // 23. RecoveryAtFinalize
        Scenario::RecoveryAtFinalize => {
            if req_id <= 1 {
                // First request cuts off right before completion
                let headers = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Accept-Ranges: bytes\r\n\
                     Content-Length: {}\r\n\
                     Connection: close\r\n\r\n",
                    total_size
                );
                stream.write_all(headers.as_bytes()).await?;
                if !is_head {
                    let cutoff = (total_size * 9) / 10;
                    let body = generate_bytes(cutoff);
                    stream.write_all(&body).await?;
                    stream.flush().await?;
                }
                let _ = stream.shutdown().await;
                Ok(false)
            } else {
                // Resumed request succeeds
                if let Some(range_str) = range_header {
                    if let Some((start, end)) = parse_range(range_str, total_size) {
                        let len = end - start + 1;
                        let headers = format!(
                            "HTTP/1.1 206 Partial Content\r\n\
                             Accept-Ranges: bytes\r\n\
                             Content-Range: bytes {}-{}/{}\r\n\
                             Content-Length: {}\r\n\
                             Connection: keep-alive\r\n\r\n",
                            start, end, total_size, len
                        );
                        stream.write_all(headers.as_bytes()).await?;
                        if !is_head {
                            let body = generate_bytes_offset(start, len);
                            stream.write_all(&body).await?;
                        }
                        return Ok(true);
                    }
                }
                let headers = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Length: {}\r\n\
                     Connection: keep-alive\r\n\r\n",
                    total_size
                );
                stream.write_all(headers.as_bytes()).await?;
                if !is_head {
                    let body = generate_bytes(total_size);
                    stream.write_all(&body).await?;
                }
                Ok(true)
            }
        }

        // 24. ProxyTLS
        Scenario::ProxyTLS => {
            let headers = "HTTP/1.1 407 Proxy Authentication Required\r\n\
                           Proxy-Authenticate: Basic realm=\"SaylProxy\"\r\n\
                           Via: 1.1 sayl-proxy\r\n\
                           Content-Length: 0\r\n\
                           Connection: close\r\n\r\n";
            stream.write_all(headers.as_bytes()).await?;
            Ok(false)
        }

        // 25. LimitsOrigin
        Scenario::LimitsOrigin => {
            if active_conns > 1 {
                let headers = "HTTP/1.1 503 Service Unavailable\r\n\
                               Retry-After: 10\r\n\
                               Content-Length: 0\r\n\
                               Connection: close\r\n\r\n";
                stream.write_all(headers.as_bytes()).await?;
                Ok(false)
            } else {
                let headers = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Length: {}\r\n\
                     Connection: keep-alive\r\n\r\n",
                    total_size
                );
                stream.write_all(headers.as_bytes()).await?;
                if !is_head {
                    let body = generate_bytes(total_size);
                    stream.write_all(&body).await?;
                }
                Ok(true)
            }
        }

        // 26. FileTruncation
        Scenario::FileTruncation => {
            let shrunk_size = total_size / 2;
            if let Some(range_str) = range_header {
                if let Some((start, req_end)) = parse_range(range_str, total_size) {
                    if start >= shrunk_size {
                        let headers = format!(
                            "HTTP/1.1 416 Range Not Satisfiable\r\n\
                             Content-Range: bytes */{}\r\n\
                             Content-Length: 0\r\n\
                             Connection: keep-alive\r\n\r\n",
                            shrunk_size
                        );
                        stream.write_all(headers.as_bytes()).await?;
                        return Ok(true);
                    }
                    let end = req_end.min(shrunk_size - 1);
                    let len = end - start + 1;
                    let headers = format!(
                        "HTTP/1.1 206 Partial Content\r\n\
                         Accept-Ranges: bytes\r\n\
                         Content-Range: bytes {}-{}/{}\r\n\
                         Content-Length: {}\r\n\
                         Connection: keep-alive\r\n\r\n",
                        start, end, shrunk_size, len
                    );
                    stream.write_all(headers.as_bytes()).await?;
                    if !is_head {
                        let body = generate_bytes_offset(start, len);
                        stream.write_all(&body).await?;
                    }
                    return Ok(true);
                }
            }
            let headers = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Length: {}\r\n\
                 Connection: keep-alive\r\n\r\n",
                shrunk_size
            );
            stream.write_all(headers.as_bytes()).await?;
            if !is_head {
                let body = generate_bytes(shrunk_size);
                stream.write_all(&body).await?;
            }
            Ok(true)
        }

        // 27. Pooling
        Scenario::Pooling => {
            let headers = format!(
                "HTTP/1.1 200 OK\r\n\
                 Accept-Ranges: bytes\r\n\
                 Content-Length: {}\r\n\
                 Connection: keep-alive\r\n\r\n",
                total_size
            );
            stream.write_all(headers.as_bytes()).await?;
            if !is_head {
                let body = generate_bytes(total_size);
                stream.write_all(&body).await?;
            }
            Ok(true)
        }
    }
}

/// Streams body data in configurable chunk sizes, with optional delays.
async fn write_streamed_body(
    stream: &mut TcpStream,
    data: &[u8],
    chunk_size: usize,
    delay: Option<Duration>,
) -> Result<(), std::io::Error> {
    let size = chunk_size.max(1);
    for chunk in data.chunks(size) {
        stream.write_all(chunk).await?;
        if let Some(d) = delay {
            tokio::time::sleep(d).await;
        }
    }
    stream.flush().await?;
    Ok(())
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_range_standard() {
        let (s, e) = parse_range("bytes=0-499", 1000).unwrap();
        assert_eq!((s, e), (0, 499));

        let (s, e) = parse_range("bytes=500-999", 1000).unwrap();
        assert_eq!((s, e), (500, 999));
    }

    #[test]
    fn test_parse_range_open_ended() {
        let (s, e) = parse_range("bytes=750-", 1000).unwrap();
        assert_eq!((s, e), (750, 999));
    }

    #[test]
    fn test_parse_range_suffix() {
        let (s, e) = parse_range("bytes=-100", 1000).unwrap();
        assert_eq!((s, e), (900, 999));
    }

    #[test]
    fn test_parse_range_invalid() {
        assert!(parse_range("bytes=1000-1500", 1000).is_none());
        assert!(parse_range("bytes=500-200", 1000).is_none());
        assert!(parse_range("invalid", 1000).is_none());
    }

    #[test]
    fn test_server_start_and_shutdown() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let mut server = HttpFixtureServer::new();
            let addr = server.start().await.expect("Failed to start server");
            assert_ne!(addr.port(), 0);
            assert_eq!(server.local_addr(), Some(addr));
            assert!(server.url().unwrap().starts_with("http://127.0.0.1:"));

            server.shutdown();
            assert_eq!(server.local_addr(), None);
        });
    }

    #[test]
    fn test_server_with_scenario() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let server = HttpFixtureServer::with_scenario("RetryAfter429");
            let cfg = server.config.read().await;
            assert_eq!(cfg.scenario, Scenario::RetryAfter429);
        });
    }

    #[test]
    fn test_request_recording_and_verification() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let server = HttpFixtureServer::new();
            server.record_request(RecordedRequest {
                method: "GET".to_string(),
                path: "/sample.bin".to_string(),
                headers: vec![("Range".to_string(), "bytes=0-100".to_string())],
                range_header: Some("bytes=0-100".to_string()),
                timestamp_ms: 1000,
            });

            let expected = vec![ExpectedRequest::get("/sample.bin").with_range("bytes=0-100")];
            assert!(server.verify_requests(&expected).await.is_ok());

            let wrong_method = vec![ExpectedRequest::head("/sample.bin")];
            assert!(server.verify_requests(&wrong_method).await.is_err());

            let wrong_path = vec![ExpectedRequest::get("/other.bin")];
            assert!(server.verify_requests(&wrong_path).await.is_err());
        });
    }

    async fn raw_http_exchange(addr: SocketAddr, request: &str) -> (String, Vec<u8>) {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut resp = Vec::new();
        let _ = stream.read_to_end(&mut resp).await;
        let header_end = find_subsequence(&resp, b"\r\n\r\n").unwrap_or(resp.len());
        let header_str = String::from_utf8_lossy(&resp[..header_end]).to_string();
        let body = if header_end + 4 <= resp.len() {
            resp[header_end + 4..].to_vec()
        } else {
            Vec::new()
        };
        (header_str, body)
    }

    #[test]
    fn test_http_range_request_ok206() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
            server.set_resource_size(1000);
            let addr = server.start().await.unwrap();

            let req = "GET /test.bin HTTP/1.1\r\n\
                       Host: localhost\r\n\
                       Range: bytes=0-99\r\n\
                       Connection: close\r\n\r\n";

            let (headers, body) = raw_http_exchange(addr, req).await;
            assert!(headers.starts_with("HTTP/1.1 206 Partial Content"));
            assert!(headers.contains("Content-Range: bytes 0-99/1000"));
            assert!(headers.contains("Accept-Ranges: bytes"));
            assert_eq!(body.len(), 100);
            assert!(crate::body_gen::verify_pattern(&body, 0));

            server.shutdown();
        });
    }

    #[test]
    fn test_http_head_rejected_scenario() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let mut server = HttpFixtureServer::with_scenario(Scenario::HeadRejected);
            let addr = server.start().await.unwrap();

            let req = "HEAD /test.bin HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            let (headers, body) = raw_http_exchange(addr, req).await;
            assert!(headers.starts_with("HTTP/1.1 405 Method Not Allowed"));
            assert!(body.is_empty());

            server.shutdown();
        });
    }

    #[test]
    fn test_http_retry_after_429_scenario() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let mut server = HttpFixtureServer::with_scenario(Scenario::RetryAfter429);
            let addr = server.start().await.unwrap();

            let req = "GET /rate-limited HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            let (headers, _) = raw_http_exchange(addr, req).await;
            assert!(headers.starts_with("HTTP/1.1 429 Too Many Requests"));
            assert!(headers.contains("Retry-After: 5"));

            server.shutdown();
        });
    }

    #[test]
    fn test_http_redirect_with_secrets_scenario() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let mut server = HttpFixtureServer::with_scenario(Scenario::RedirectWithSecrets);
            let addr = server.start().await.unwrap();

            let req = "GET /redirect HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            let (headers, _) = raw_http_exchange(addr, req).await;
            assert!(headers.starts_with("HTTP/1.1 302 Found"));
            assert!(headers.contains("Location: "));

            server.shutdown();
        });
    }

    #[test]
    fn test_http_range_not_satisfiable_416() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let mut server = HttpFixtureServer::with_scenario(Scenario::RangeNotSatisfiable416);
            server.set_resource_size(500);
            let addr = server.start().await.unwrap();

            let req = "GET /test.bin HTTP/1.1\r\nHost: localhost\r\nRange: bytes=600-700\r\nConnection: close\r\n\r\n";
            let (headers, body) = raw_http_exchange(addr, req).await;
            assert!(headers.starts_with("HTTP/1.1 416 Range Not Satisfiable"));
            assert!(headers.contains("Content-Range: bytes */500"));
            assert!(body.is_empty());

            server.shutdown();
        });
    }
}

//! Origin concurrency budget and rate limiting for SAYL.
//!
//! Provides an in-memory, testable origin budget with configurable concurrent
//! request limits and per-origin request rate limits. Time source is injected
//! for deterministic testing.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Time source trait for testable time-dependent logic.
pub trait TimeSource: Send + Sync {
    /// Returns the current instant.
    fn now(&self) -> Instant;
}

/// Real wall-clock time source.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemTimeSource;

impl TimeSource for SystemTimeSource {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Manually controlled time source for deterministic testing.
#[derive(Debug, Clone)]
pub struct ManualTimeSource {
    now: Arc<Mutex<Instant>>,
}

impl Default for ManualTimeSource {
    fn default() -> Self {
        Self {
            now: Arc::new(Mutex::new(Instant::now())),
        }
    }
}

impl ManualTimeSource {
    /// Creates a new manual time source starting at a specific instant.
    pub fn with_start(instant: Instant) -> Self {
        Self {
            now: Arc::new(Mutex::new(instant)),
        }
    }

    /// Advances the clock by the given duration.
    pub fn advance(&self, delta: Duration) {
        let mut now = self.now.lock().unwrap();
        *now = now.checked_add(delta).unwrap_or(*now);
    }

    /// Sets the clock to a specific instant.
    pub fn set(&self, instant: Instant) {
        let mut now = self.now.lock().unwrap();
        *now = instant;
    }

    /// Returns a handle that can be shared to advance time from multiple places.
    pub fn handle(&self) -> ManualTimeHandle {
        ManualTimeHandle {
            now: Arc::clone(&self.now),
        }
    }
}

/// Handle to advance a `ManualTimeSource`.
#[derive(Debug, Clone)]
pub struct ManualTimeHandle {
    now: Arc<Mutex<Instant>>,
}

impl ManualTimeHandle {
    /// Advances the clock by the given duration.
    pub fn advance(&self, delta: Duration) {
        let mut now = self.now.lock().unwrap();
        *now = now.checked_add(delta).unwrap_or(*now);
    }
}

impl TimeSource for ManualTimeSource {
    fn now(&self) -> Instant {
        *self.now.lock().unwrap()
    }
}

/// Per-origin budget tracking.
#[derive(Debug, Clone)]
struct OriginState {
    /// Currently active (acquired but not released) requests.
    active: usize,
    /// Timestamps of recent request starts for rate limiting.
    request_times: Vec<Instant>,
}

/// Budget for controlling concurrency and request rate per origin.
///
/// An "origin" is defined as scheme + host + port (e.g., "https://example.com:443").
#[derive(Clone)]
pub struct OriginBudget {
    /// Maximum concurrent requests per origin.
    per_host: usize,
    /// Maximum requests per second per origin.
    per_origin_requests_per_sec: u32,
    /// Shared state across all origins.
    state: Arc<Mutex<HashMap<String, OriginState>>>,
    /// Time source for rate limit window calculations.
    time_source: Arc<dyn TimeSource>,
}

impl std::fmt::Debug for OriginBudget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OriginBudget")
            .field("per_host", &self.per_host)
            .field("per_origin_requests_per_sec", &self.per_origin_requests_per_sec)
            .field("active_origins", &self.state.lock().unwrap().len())
            .finish()
    }
}

impl OriginBudget {
    /// Creates a new origin budget with the given limits and system time source.
    ///
    /// - `per_host`: Maximum concurrent requests allowed for a single origin.
    /// - `per_origin_requests_per_sec`: Maximum request starts per second per origin.
    pub fn new(per_host: usize, per_origin_requests_per_sec: u32) -> Self {
        Self {
            per_host,
            per_origin_requests_per_sec,
            state: Arc::new(Mutex::new(HashMap::new())),
            time_source: Arc::new(SystemTimeSource),
        }
    }

    /// Creates a new origin budget with a custom time source (for testing).
    pub fn with_time_source(
        per_host: usize,
        per_origin_requests_per_sec: u32,
        time_source: Arc<dyn TimeSource>,
    ) -> Self {
        Self {
            per_host,
            per_origin_requests_per_sec,
            state: Arc::new(Mutex::new(HashMap::new())),
            time_source,
        }
    }

    /// Attempts to acquire a slot for the given origin.
    ///
    /// Returns `true` if the request may proceed, `false` if the origin's
    /// concurrency or rate limit would be exceeded.
    pub fn try_acquire(&self, origin: &str) -> bool {
        let now = self.time_source.now();
        let mut state = self.state.lock().unwrap();

        let entry = state.entry(origin.to_string()).or_insert_with(|| OriginState {
            active: 0,
            request_times: Vec::new(),
        });

        // Clean up old request timestamps outside the 1-second window
        let window_start = now.checked_sub(Duration::from_secs(1)).unwrap_or(now);
        entry.request_times.retain(|&t| t >= window_start);

        // Check concurrency limit
        if entry.active >= self.per_host {
            return false;
        }

        // Check rate limit
        if entry.request_times.len() >= self.per_origin_requests_per_sec as usize {
            return false;
        }

        // Acquire
        entry.active += 1;
        entry.request_times.push(now);
        true
    }

    /// Releases a previously acquired slot for the given origin.
    ///
    /// Should be called when a request completes (success or failure).
    /// Panics if called more times than `try_acquire` succeeded for this origin
    /// (indicates a logic error in the caller).
    pub fn release(&self, origin: &str) {
        let mut state = self.state.lock().unwrap();

        if let Some(entry) = state.get_mut(origin) {
            assert!(entry.active > 0, "release called without matching acquire for origin '{}'", origin);
            entry.active -= 1;

            // Clean up empty entries to prevent memory growth
            if entry.active == 0 && entry.request_times.is_empty() {
                state.remove(origin);
            }
        } else {
            // This could happen if release is called without acquire, or after cleanup
            // In production we might want to log a warning, but for now we allow it
            // as a no-op to be resilient.
        }
    }

    /// Returns the current number of active requests for an origin.
    pub fn active_count(&self, origin: &str) -> usize {
        let state = self.state.lock().unwrap();
        state.get(origin).map(|e| e.active).unwrap_or(0)
    }

    /// Returns the current request rate (requests in the last second) for an origin.
    pub fn current_rate(&self, origin: &str) -> usize {
        let now = self.time_source.now();
        let state = self.state.lock().unwrap();
        if let Some(entry) = state.get(origin) {
            let window_start = now.checked_sub(Duration::from_secs(1)).unwrap_or(now);
            entry.request_times.iter().filter(|&&t| t >= window_start).count()
        } else {
            0
        }
    }

    /// Resets all budget state (for testing).
    pub fn reset(&self) {
        let mut state = self.state.lock().unwrap();
        state.clear();
    }
}

/// Extracts the origin (scheme + host + port) from a URL for budget tracking.
pub fn origin_from_url(url: &url::Url) -> String {
    let scheme = url.scheme();
    let host = url.host_str().unwrap_or("");
    let port = url.port_or_known_default().unwrap_or(0);
    format!("{}://{}:{}", scheme, host, port)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_origin_budget_allows_per_host_concurrent() {
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

    #[test]
    fn test_origin_budget_releases_correctly() {
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

    #[test]
    fn test_origin_budget_enforces_rate_limit() {
        let time_source = Arc::new(ManualTimeSource::default());
        let budget = OriginBudget::with_time_source(10, 3, Arc::clone(&time_source) as Arc<dyn TimeSource>);
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

    #[test]
    fn test_origin_budget_separate_origins_independent() {
        let budget = OriginBudget::new(2, 100);
        let origin1 = "https://example.com:443";
        let origin2 = "https://api.example.com:443";
        let origin3 = "http://example.com:80";

        assert!(budget.try_acquire(origin1));
        assert!(budget.try_acquire(origin1));
        assert!(!budget.try_acquire(origin1));

        // origin2 has its own budget
        assert!(budget.try_acquire(origin2));
        assert!(budget.try_acquire(origin2));
        assert!(!budget.try_acquire(origin2));

        // origin3 (different scheme) has its own budget
        assert!(budget.try_acquire(origin3));
        assert!(budget.try_acquire(origin3));
        assert!(!budget.try_acquire(origin3));
    }

    #[test]
    fn test_origin_budget_release_without_acquire_is_noop() {
        let budget = OriginBudget::new(2, 100);
        let origin = "https://example.com:443";

        // Should not panic
        budget.release(origin);
        budget.release(origin);
    }

    #[test]
    fn test_origin_budget_with_time_source_manual_control() {
        let time_source = ManualTimeSource::with_start(Instant::now());
        let handle = time_source.handle();
        let budget = OriginBudget::with_time_source(5, 2, Arc::new(time_source) as Arc<dyn TimeSource>);
        let origin = "https://test.com:443";

        assert!(budget.try_acquire(origin));
        assert!(budget.try_acquire(origin));
        assert!(!budget.try_acquire(origin)); // rate limit hit

        handle.advance(Duration::from_secs(2));
        assert!(budget.try_acquire(origin));
        assert!(budget.try_acquire(origin));
        assert!(!budget.try_acquire(origin));
    }

    #[test]
    fn test_origin_from_url() {
        let url = url::Url::parse("https://example.com/file").unwrap();
        assert_eq!(origin_from_url(&url), "https://example.com:443");

        let url = url::Url::parse("http://example.com:8080/file").unwrap();
        assert_eq!(origin_from_url(&url), "http://example.com:8080");

        let url = url::Url::parse("https://192.168.1.1:8443/path").unwrap();
        assert_eq!(origin_from_url(&url), "https://192.168.1.1:8443");
    }
}
//! Retry policy with exponential backoff, jitter, and `Retry-After` support.
//!
//! Retries are only ever attempted for transient conditions. Cancellation and
//! fatal errors (404, 401, validation failures) are never retried, so a retry loop
//! can never mask a permanent failure or spin after the user asked to stop.
//!
//! ```
//! use sayl_http::retry::RetryPolicy;
//! use std::time::Duration;
//!
//! let policy = RetryPolicy::deterministic(); // jitter off, for tests
//! assert_eq!(policy.backoff(0), Duration::from_millis(250));
//! assert_eq!(policy.backoff(1), Duration::from_millis(500));
//! ```

use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::error::HttpError;

/// Bounded exponential backoff configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Maximum number of attempts, including the first one.
    pub max_attempts: u32,
    /// Delay before the first retry.
    pub base_delay: Duration,
    /// Upper bound for any single delay.
    pub max_delay: Duration,
    /// Whether to spread delays with jitter.
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            base_delay: Duration::from_millis(250),
            max_delay: Duration::from_secs(30),
            jitter: true,
        }
    }
}

impl RetryPolicy {
    /// A policy with jitter disabled, giving exact delays for tests.
    pub fn deterministic() -> Self {
        Self {
            jitter: false,
            ..Self::default()
        }
    }

    /// Exponential delay for a zero-based retry attempt, capped at `max_delay`.
    ///
    /// With `jitter` enabled the result is a full-jitter value in
    /// `[delay/2, delay]`, derived deterministically from the attempt number so a
    /// single run is reproducible.
    pub fn backoff(&self, attempt: u32) -> Duration {
        let exp = self
            .base_delay
            .as_millis()
            .saturating_mul(1u128 << attempt.min(32));
        // clippy::unnecessary_cast is a false positive across toolchains: on
        // newer `Duration` it is a no-op, on older ones it converts u64 to u128.
        #[allow(clippy::unnecessary_cast)]
        let capped = Duration::from_millis(exp.min(self.max_delay.as_millis() as u128) as u64);
        if !self.jitter {
            return capped;
        }
        let half = capped.as_millis() as u64 / 2;
        // Deterministic pseudo-jitter: no RNG dependency, no global state.
        let spread = half.max(1);
        let offset = (attempt as u64).wrapping_mul(2_654_435_761) % spread;
        capped
            .saturating_sub(Duration::from_millis(offset))
            .max(Duration::from_millis(1))
    }

    /// Parses a `Retry-After` header value (delta-seconds or HTTP-date).
    pub fn parse_retry_after(value: &str, now: DateTime<Utc>) -> Option<Duration> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Ok(seconds) = trimmed.parse::<u64>() {
            return Some(Duration::from_secs(seconds));
        }
        let when = DateTime::parse_from_rfc2822(trimmed)
            .ok()
            .map(|datetime| datetime.with_timezone(&Utc))?;
        let delta = when.signed_duration_since(now);
        if delta.num_seconds() <= 0 {
            Some(Duration::ZERO)
        } else {
            Some(Duration::from_secs(delta.num_seconds() as u64))
        }
    }

    /// Delay before the next attempt: the server's `Retry-After` wins, capped by
    /// `max_delay`.
    pub fn delay_for_attempt(&self, attempt: u32, retry_after: Option<Duration>) -> Duration {
        match retry_after {
            Some(server) => server.min(self.max_delay),
            None => self.backoff(attempt),
        }
    }

    /// Whether another attempt is permitted after `attempts_made` attempts.
    pub fn may_retry(&self, attempts_made: u32) -> bool {
        attempts_made < self.max_attempts
    }
}

/// Status codes that justify a retry.
pub fn is_retryable_status(status: u16) -> bool {
    matches!(status, 408 | 425 | 429 | 500 | 502 | 503 | 504)
}

/// Whether the given error should be retried, reusing the crate's own
/// retryable/fatal/cancelled classification.
pub fn should_retry(err: &HttpError) -> bool {
    if err.is_user_cancelled() || err.is_fatal() {
        return false;
    }
    if let Some(status) = status_of(err) {
        if !is_retryable_status(status) {
            return false;
        }
    }
    err.is_retryable()
}

/// Extracts an HTTP status code when the error carries one.
pub fn status_of(err: &HttpError) -> Option<u16> {
    match err {
        HttpError::Status { status, .. } => Some(*status),
        HttpError::Range { status, .. } => *status,
        _ => None,
    }
}

#[cfg(test)]
impl RetryPolicy {
    /// Uncapped exponential delay, used by tests to bound jittered values.
    pub(crate) fn backoff_cap(&self, attempt: u32) -> Duration {
        let exp = self
            .base_delay
            .as_millis()
            .saturating_mul(1u128 << attempt.min(32));
        Duration::from_millis(exp.min(self.max_delay.as_millis()) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_backoff_grows_and_caps() {
        let policy = RetryPolicy::deterministic();
        assert_eq!(policy.backoff(0), Duration::from_millis(250));
        assert_eq!(policy.backoff(1), Duration::from_millis(500));
        assert_eq!(policy.backoff(2), Duration::from_millis(1000));
        assert_eq!(policy.backoff(3), Duration::from_millis(2000));
        // Capped at 30s no matter how large the attempt index grows.
        assert_eq!(policy.backoff(20), Duration::from_secs(30));
        assert_eq!(policy.backoff(64), Duration::from_secs(30));
    }

    #[test]
    fn jittered_backoff_stays_within_bounds() {
        let policy = RetryPolicy::default();
        for attempt in 0..8 {
            let delay = policy.backoff(attempt);
            let ceiling = policy.backoff_cap(attempt);
            assert!(delay >= Duration::from_millis(1), "delay must be positive");
            assert!(delay <= ceiling, "delay {delay:?} exceeds cap {ceiling:?}");
        }
    }

    #[test]
    fn parse_retry_after_seconds_and_http_date() {
        let now = DateTime::parse_from_rfc2822("Wed, 21 Oct 2015 07:28:00 GMT")
            .unwrap()
            .with_timezone(&Utc);

        assert_eq!(
            RetryPolicy::parse_retry_after("120", now),
            Some(Duration::from_secs(120))
        );
        assert_eq!(
            RetryPolicy::parse_retry_after("Wed, 21 Oct 2015 07:28:30 GMT", now),
            Some(Duration::from_secs(30))
        );
        // A date in the past means "retry now".
        assert_eq!(
            RetryPolicy::parse_retry_after("Wed, 21 Oct 2015 07:27:00 GMT", now),
            Some(Duration::ZERO)
        );
        assert_eq!(RetryPolicy::parse_retry_after("", now), None);
        assert_eq!(RetryPolicy::parse_retry_after("not-a-date", now), None);
    }

    #[test]
    fn retry_after_wins_and_is_capped() {
        let policy = RetryPolicy::deterministic();
        assert_eq!(
            policy.delay_for_attempt(0, Some(Duration::from_secs(5))),
            Duration::from_secs(5)
        );
        assert_eq!(
            policy.delay_for_attempt(0, Some(Duration::from_secs(600))),
            policy.max_delay
        );
        assert_eq!(
            policy.delay_for_attempt(2, None),
            Duration::from_millis(1000)
        );
    }

    #[test]
    fn retry_budget_is_bounded() {
        let policy = RetryPolicy::default();
        assert!(policy.may_retry(0));
        assert!(policy.may_retry(4));
        assert!(!policy.may_retry(5));
        assert!(!policy.may_retry(99));
    }

    #[test]
    fn retryable_status_classification() {
        for status in [408u16, 425, 429, 500, 502, 503, 504] {
            assert!(is_retryable_status(status), "{status} should be retryable");
        }
        for status in [200u16, 206, 301, 400, 401, 403, 404, 410, 416] {
            assert!(
                !is_retryable_status(status),
                "{status} must not be retryable"
            );
        }
    }

    #[test]
    fn cancelled_is_never_retried() {
        let err = HttpError::Cancelled {
            reason: "user pressed cancel".into(),
        };
        assert!(!should_retry(&err));
    }

    #[test]
    fn fatal_status_is_not_retried() {
        let err = HttpError::Status {
            status: 404,
            message: "Not Found".into(),
            retry_after_secs: None,
        };
        assert!(!should_retry(&err));
    }

    #[test]
    fn transient_status_is_retried() {
        let err = HttpError::Status {
            status: 503,
            message: "Service Unavailable".into(),
            retry_after_secs: Some(30),
        };
        assert!(should_retry(&err));
        assert_eq!(err.retry_after_secs(), Some(30));
        assert_eq!(status_of(&err), Some(503));
    }

    #[test]
    fn truncated_body_is_retryable() {
        let err = HttpError::Truncated {
            expected: 1024,
            actual: 512,
        };
        assert!(should_retry(&err));
        assert_eq!(status_of(&err), None);
    }
}

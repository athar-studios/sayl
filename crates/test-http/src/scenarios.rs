//! Failure mode scenarios specified in MASTER-PLAN §8.
//!
//! Provides all 27 mandatory download simulation scenarios:
//! 1. Ok206
//! 2. NoAcceptRanges
//! 3. IgnoredRange200
//! 4. BadContentRange
//! 5. BodyTruncation
//! 6. ConnectionReset
//! 7. ETagChanged
//! 8. WeakETagOnly
//! 9. NoValidator
//! 10. RangeNotSatisfiable416 (416RangeNotSatisfiable)
//! 11. HeadRejected
//! 12. UnknownSize
//! 13. EmptyFile
//! 14. UnexpectedEncoding
//! 15. RetryAfter429 (429RetryAfter)
//! 16. RedirectWithSecrets
//! 17. NoSpace
//! 18. PermissionDenied
//! 19. MaliciousFilename
//! 20. PauseCancelDuringProbe
//! 21. SlowWorker
//! 22. OldLease
//! 23. RecoveryAtFinalize
//! 24. ProxyTLS
//! 25. LimitsOrigin
//! 26. FileTruncation
//! 27. Pooling

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

/// All 27 failure modes and protocol conditions from MASTER-PLAN §8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Scenario {
    /// 1. Standard valid 206 Partial Content with Content-Range and Accept-Ranges.
    Ok206,
    /// 2. 200 OK without Accept-Ranges header, ignoring Range header.
    NoAcceptRanges,
    /// 3. 200 OK returning full file body despite a Range request, without Content-Range.
    IgnoredRange200,
    /// 4. 206 Partial Content with a malformed or corrupted Content-Range header.
    BadContentRange,
    /// 5. Body stream abruptly cut off before reaching declared Content-Length or Content-Range.
    BodyTruncation,
    /// 6. Immediate TCP reset (RST / socket close) on connection or early in transfer.
    ConnectionReset,
    /// 7. ETag changes between consecutive requests (invalidating concurrent/resumed chunks).
    ETagChanged,
    /// 8. Only a weak ETag (prefix `W/`) is provided.
    WeakETagOnly,
    /// 9. Neither ETag nor Last-Modified validators are provided.
    NoValidator,
    /// 10. 416 Range Not Satisfiable when range is beyond resource boundaries.
    RangeNotSatisfiable416,
    /// 11. HEAD requests are rejected with 405 Method Not Allowed or 501 Not Implemented.
    HeadRejected,
    /// 12. 200 OK without Content-Length and without chunked transfer encoding (close delimited).
    UnknownSize,
    /// 13. Empty file response (Content-Length: 0).
    EmptyFile,
    /// 14. Unexpected Content-Encoding (e.g. gzip) when identity was requested.
    UnexpectedEncoding,
    /// 15. 429 Too Many Requests with a Retry-After header.
    RetryAfter429,
    /// 16. Cross-origin redirect testing secret/credential leak protection.
    RedirectWithSecrets,
    /// 17. Simulates storage full / ENOSPC (507 Insufficient Storage).
    NoSpace,
    /// 18. Simulates permission denied / EACCES (403 Forbidden).
    PermissionDenied,
    /// 19. Malicious path traversal or Arabic Unicode filename in Content-Disposition.
    MaliciousFilename,
    /// 20. Stalls or hangs during probe phase to test pause/cancellation.
    PauseCancelDuringProbe,
    /// 21. Deliberately slow streaming worker with artificial inter-chunk delay.
    SlowWorker,
    /// 22. Expired chunk lease response (412 Precondition Failed or 410 Gone).
    OldLease,
    /// 23. Failure right before finalization that succeeds upon resumption.
    RecoveryAtFinalize,
    /// 24. Proxy or TLS authentication challenge (407 Proxy Authentication Required).
    ProxyTLS,
    /// 25. Origin concurrency / rate limits exceeded (503 Service Unavailable).
    LimitsOrigin,
    /// 26. Remote resource truncated on resume (file size smaller than prior probe).
    FileTruncation,
    /// 27. HTTP/1.1 connection pooling / Keep-Alive reuse across requests.
    Pooling,
}

impl Scenario {
    /// Canonical identifier for the scenario.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ok206 => "Ok206",
            Self::NoAcceptRanges => "NoAcceptRanges",
            Self::IgnoredRange200 => "IgnoredRange200",
            Self::BadContentRange => "BadContentRange",
            Self::BodyTruncation => "BodyTruncation",
            Self::ConnectionReset => "ConnectionReset",
            Self::ETagChanged => "ETagChanged",
            Self::WeakETagOnly => "WeakETagOnly",
            Self::NoValidator => "NoValidator",
            Self::RangeNotSatisfiable416 => "416RangeNotSatisfiable",
            Self::HeadRejected => "HeadRejected",
            Self::UnknownSize => "UnknownSize",
            Self::EmptyFile => "EmptyFile",
            Self::UnexpectedEncoding => "UnexpectedEncoding",
            Self::RetryAfter429 => "429RetryAfter",
            Self::RedirectWithSecrets => "RedirectWithSecrets",
            Self::NoSpace => "NoSpace",
            Self::PermissionDenied => "PermissionDenied",
            Self::MaliciousFilename => "MaliciousFilename",
            Self::PauseCancelDuringProbe => "PauseCancelDuringProbe",
            Self::SlowWorker => "SlowWorker",
            Self::OldLease => "OldLease",
            Self::RecoveryAtFinalize => "RecoveryAtFinalize",
            Self::ProxyTLS => "ProxyTLS",
            Self::LimitsOrigin => "LimitsOrigin",
            Self::FileTruncation => "FileTruncation",
            Self::Pooling => "Pooling",
        }
    }

    /// Human-readable description of the failure mode or behavior.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Ok206 => "Standard 206 Partial Content with valid Content-Range and Accept-Ranges",
            Self::NoAcceptRanges => "200 OK without Accept-Ranges header, ignoring client Range",
            Self::IgnoredRange200 => "200 OK full body response despite explicit Range header",
            Self::BadContentRange => "206 Partial Content with malformed or out-of-bounds Content-Range",
            Self::BodyTruncation => "Body prematurely terminated before reaching declared size",
            Self::ConnectionReset => "Abrupt TCP socket reset or closure during transfer",
            Self::ETagChanged => "ETag value changes between consecutive probe/download requests",
            Self::WeakETagOnly => "Weak validator (W/\"...\") returned without strong validator",
            Self::NoValidator => "Response lacks both ETag and Last-Modified headers",
            Self::RangeNotSatisfiable416 => "416 Range Not Satisfiable returned for out-of-bounds range",
            Self::HeadRejected => "HEAD probe request rejected with 405 Method Not Allowed or 501",
            Self::UnknownSize => "200 OK stream without Content-Length and without chunked encoding",
            Self::EmptyFile => "200 OK response with 0-byte Content-Length",
            Self::UnexpectedEncoding => "Unexpected Content-Encoding: gzip when identity was requested",
            Self::RetryAfter429 => "429 Too Many Requests response with Retry-After header",
            Self::RedirectWithSecrets => "Cross-domain redirect attempting to capture Authorization/Cookie",
            Self::NoSpace => "Simulated storage space exhaustion (507 Insufficient Storage)",
            Self::PermissionDenied => "Simulated access denial (403 Forbidden)",
            Self::MaliciousFilename => "Malicious path traversal or Arabic Unicode in Content-Disposition",
            Self::PauseCancelDuringProbe => "Probe hangs indefinitely to test cancellation handling",
            Self::SlowWorker => "Artificially throttled streaming to test slow chunk workers",
            Self::OldLease => "Precondition failure simulating expired lease (412 / 410)",
            Self::RecoveryAtFinalize => "Transfer drop just before finalize, allowing successful resume",
            Self::ProxyTLS => "Proxy TLS challenge (407 Proxy Authentication Required / Via)",
            Self::LimitsOrigin => "Origin concurrency limit reached (503 Service Unavailable)",
            Self::FileTruncation => "Resource size shrunk upon resume compared to previous probe",
            Self::Pooling => "HTTP/1.1 persistent connection reuse across multiple requests",
        }
    }

    /// Slice containing all 27 failure modes in canonical order.
    pub fn all() -> &'static [Scenario] {
        static ALL: [Scenario; 27] = [
            Scenario::Ok206,
            Scenario::NoAcceptRanges,
            Scenario::IgnoredRange200,
            Scenario::BadContentRange,
            Scenario::BodyTruncation,
            Scenario::ConnectionReset,
            Scenario::ETagChanged,
            Scenario::WeakETagOnly,
            Scenario::NoValidator,
            Scenario::RangeNotSatisfiable416,
            Scenario::HeadRejected,
            Scenario::UnknownSize,
            Scenario::EmptyFile,
            Scenario::UnexpectedEncoding,
            Scenario::RetryAfter429,
            Scenario::RedirectWithSecrets,
            Scenario::NoSpace,
            Scenario::PermissionDenied,
            Scenario::MaliciousFilename,
            Scenario::PauseCancelDuringProbe,
            Scenario::SlowWorker,
            Scenario::OldLease,
            Scenario::RecoveryAtFinalize,
            Scenario::ProxyTLS,
            Scenario::LimitsOrigin,
            Scenario::FileTruncation,
            Scenario::Pooling,
        ];
        &ALL
    }
}

impl fmt::Display for Scenario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Error returned when parsing an unrecognized scenario string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioParseError(pub String);

impl fmt::Display for ScenarioParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown scenario name: {}", self.0)
    }
}

impl std::error::Error for ScenarioParseError {}

impl FromStr for Scenario {
    type Err = ScenarioParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace(['-', '_', ' '], "");
        match normalized.as_str() {
            "ok206" | "206" => Ok(Self::Ok206),
            "noacceptranges" | "noacceptrange" => Ok(Self::NoAcceptRanges),
            "ignoredrange200" | "ignoredrange" => Ok(Self::IgnoredRange200),
            "badcontentrange" => Ok(Self::BadContentRange),
            "bodytruncation" | "truncation" => Ok(Self::BodyTruncation),
            "connectionreset" | "reset" | "rst" => Ok(Self::ConnectionReset),
            "etagchanged" => Ok(Self::ETagChanged),
            "weaketagonly" | "weaketag" => Ok(Self::WeakETagOnly),
            "novalidator" => Ok(Self::NoValidator),
            "416rangenotsatisfiable" | "rangenotsatisfiable416" | "416" | "rangenotsatisfiable" => {
                Ok(Self::RangeNotSatisfiable416)
            }
            "headrejected" | "rejecthead" => Ok(Self::HeadRejected),
            "unknownsize" | "nosize" => Ok(Self::UnknownSize),
            "emptyfile" | "empty" => Ok(Self::EmptyFile),
            "unexpectedencoding" | "unexpectedgzip" => Ok(Self::UnexpectedEncoding),
            "429retryafter" | "retryafter429" | "429" | "retryafter" => Ok(Self::RetryAfter429),
            "redirectwithsecrets" | "redirectleak" => Ok(Self::RedirectWithSecrets),
            "nospace" | "enospc" => Ok(Self::NoSpace),
            "permissiondenied" | "eacces" | "403" => Ok(Self::PermissionDenied),
            "maliciousfilename" | "maliciousname" => Ok(Self::MaliciousFilename),
            "pausecancelduringprobe" | "probehang" | "pauseprobe" => {
                Ok(Self::PauseCancelDuringProbe)
            }
            "slowworker" | "slow" => Ok(Self::SlowWorker),
            "oldlease" | "expiredlease" => Ok(Self::OldLease),
            "recoveryatfinalize" | "recoveryfinalize" => Ok(Self::RecoveryAtFinalize),
            "proxytls" | "proxy" => Ok(Self::ProxyTLS),
            "limitsorigin" | "originlimits" => Ok(Self::LimitsOrigin),
            "filetruncation" | "truncatedresume" => Ok(Self::FileTruncation),
            "pooling" | "keepalive" => Ok(Self::Pooling),
            _ => Err(ScenarioParseError(s.to_string())),
        }
    }
}

/// Dynamic configuration parameters for scenario execution.
#[derive(Debug, Clone)]
pub struct ScenarioConfig {
    pub scenario: Scenario,
    pub resource_size: usize,
    pub default_etag: String,
    pub secondary_etag: String,
    pub retry_after_seconds: u32,
    pub delay: Option<Duration>,
    pub chunk_size: usize,
    pub truncation_length: usize,
    pub redirect_location: String,
    pub malicious_filename: String,
}

impl Default for ScenarioConfig {
    fn default() -> Self {
        Self {
            scenario: Scenario::Ok206,
            resource_size: 1024 * 1024, // 1 MiB default
            default_etag: "\"sayl-etag-v1\"".to_string(),
            secondary_etag: "\"sayl-etag-v2\"".to_string(),
            retry_after_seconds: 5,
            delay: None,
            chunk_size: 64 * 1024, // 64 KiB
            truncation_length: 512,
            redirect_location: "http://127.0.0.1:9999/unauthorized-leak-target".to_string(),
            malicious_filename: "../../../etc/passwd; filename*=UTF-8''%D9%85%D9%84%D9%81_%D8%AA%D8%AC%D8%B1%D9%8A%D8%A8%D9%8A.bin".to_string(),
        }
    }
}

impl ScenarioConfig {
    pub fn new(scenario: Scenario) -> Self {
        Self {
            scenario,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_27_scenarios_count() {
        let all = Scenario::all();
        assert_eq!(all.len(), 27, "Must contain exactly 27 scenarios from MASTER-PLAN §8");
    }

    #[test]
    fn test_scenario_names_and_from_str() {
        for scenario in Scenario::all() {
            let name = scenario.name();
            let parsed = Scenario::from_str(name).unwrap_or_else(|_| panic!("Failed parsing {}", name));
            assert_eq!(*scenario, parsed);
        }
    }

    #[test]
    fn test_alternate_name_aliases() {
        assert_eq!(Scenario::from_str("416RangeNotSatisfiable").unwrap(), Scenario::RangeNotSatisfiable416);
        assert_eq!(Scenario::from_str("RangeNotSatisfiable416").unwrap(), Scenario::RangeNotSatisfiable416);
        assert_eq!(Scenario::from_str("429RetryAfter").unwrap(), Scenario::RetryAfter429);
        assert_eq!(Scenario::from_str("RetryAfter429").unwrap(), Scenario::RetryAfter429);
        assert_eq!(Scenario::from_str("connection_reset").unwrap(), Scenario::ConnectionReset);
        assert_eq!(Scenario::from_str("pause-cancel-during-probe").unwrap(), Scenario::PauseCancelDuringProbe);
    }

    #[test]
    fn test_invalid_scenario_error() {
        let err = Scenario::from_str("NonExistentScenario").unwrap_err();
        assert_eq!(err.0, "NonExistentScenario");
    }

    #[test]
    fn test_scenario_descriptions_non_empty() {
        for scenario in Scenario::all() {
            assert!(!scenario.description().is_empty());
        }
    }
}

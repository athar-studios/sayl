//! Validator-safe resume decisions for incomplete downloads.
//!
//! A resume is only safe when the server still serves the *same* representation.
//! This module encodes that contract explicitly:
//!
//! - a **weak** `ETag` (`W/"..."`) never authorises a byte-range resume, because a
//!   weak validator permits semantically equivalent but byte-different bodies;
//! - `If-Range` is built from a strong `ETag`, else `Last-Modified`, else omitted;
//! - any change of total size or loss of the validator forces a restart from zero.
//!
//! ```
//! use sayl_http::resume::{validate_resume, ResumeDecision, ResumeInfo};
//!
//! let info = ResumeInfo {
//!     url: "https://example.com/file.bin".into(),
//!     etag: Some("\"v1\"".into()),
//!     last_modified: None,
//!     total_size: Some(1024),
//!     written: 512,
//! };
//! // Same strong validator and same size: safe to resume at 512.
//! assert_eq!(
//!     validate_resume(&info, Some("\"v1\""), None, Some(1024)),
//!     ResumeDecision::ResumeFrom(512)
//! );
//! ```

use serde::{Deserialize, Serialize};

/// Which validator backs a `ResumeInfo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidatorKind {
    /// Strong entity tag, usable for `If-Range`.
    StrongEtag,
    /// Weak entity tag (`W/"..."`), never usable for `If-Range`.
    WeakEtag,
    /// `Last-Modified` timestamp only.
    LastModified,
    /// No validator at all: resume must restart.
    None,
}

/// Everything needed to attempt a byte-range resume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumeInfo {
    /// Original URL.
    pub url: String,
    /// `ETag` as received (may be weak).
    pub etag: Option<String>,
    /// `Last-Modified` as received.
    pub last_modified: Option<String>,
    /// Total resource size, when known.
    pub total_size: Option<u64>,
    /// Bytes already durably written.
    pub written: u64,
}

/// Outcome of a resume safety check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeDecision {
    /// Safe to continue from this offset.
    ResumeFrom(u64),
    /// The representation changed or is unverifiable: start again at zero.
    RestartFromZero,
    /// The partial file already holds every byte.
    AlreadyComplete,
    /// The stored state is inconsistent and cannot be used.
    Reject(&'static str),
}

/// Returns `true` when an entity tag is weak.
pub fn is_weak_etag(etag: &str) -> bool {
    etag.trim_start().starts_with("W/") || etag.trim_start().starts_with("w/")
}

/// Classifies the validator carried by `info`.
pub fn validator_kind(info: &ResumeInfo) -> ValidatorKind {
    match info.etag.as_deref() {
        Some(tag) if is_weak_etag(tag) => ValidatorKind::WeakEtag,
        Some(_) => ValidatorKind::StrongEtag,
        None => {
            if info.last_modified.is_some() {
                ValidatorKind::LastModified
            } else {
                ValidatorKind::None
            }
        }
    }
}

/// Builds the `If-Range` value, preferring a strong `ETag`.
///
/// A weak `ETag` is never returned, per RFC 9110 §13.1.5: `If-Range` requires a
/// strong validator to guarantee byte-identical ranges.
pub fn if_range_value(info: &ResumeInfo) -> Option<String> {
    match info.etag.as_deref() {
        Some(tag) if !is_weak_etag(tag) => Some(tag.to_string()),
        _ => info.last_modified.clone(),
    }
}

/// Decides whether a stored partial download may be resumed against a fresh probe.
pub fn validate_resume(
    info: &ResumeInfo,
    fresh_etag: Option<&str>,
    fresh_last_modified: Option<&str>,
    fresh_total: Option<u64>,
) -> ResumeDecision {
    if let (Some(known), Some(fresh)) = (info.total_size, fresh_total) {
        if known != fresh {
            return ResumeDecision::RestartFromZero;
        }
    }
    if let (Some(total), true) = (info.total_size.or(fresh_total), info.written > 0) {
        if info.written > total {
            return ResumeDecision::Reject("written bytes exceed total size");
        }
        if info.written == total {
            return ResumeDecision::AlreadyComplete;
        }
    }

    let stored_validator = info.etag.is_some() || info.last_modified.is_some();
    let fresh_validator = fresh_etag.is_some() || fresh_last_modified.is_some();

    // A validator present on one side only means the server changed its behaviour
    // or the stored state is stale; neither is safe.
    if stored_validator != fresh_validator || !stored_validator {
        return ResumeDecision::RestartFromZero;
    }

    if let Some(stored) = info.etag.as_deref() {
        let fresh = match fresh_etag {
            Some(value) => value,
            None => return ResumeDecision::RestartFromZero,
        };
        if stored != fresh {
            return ResumeDecision::RestartFromZero;
        }
        // Equal entity tags still cannot authorise a resume when either side is weak,
        // because a weak tag does not promise byte equality.
        if is_weak_etag(stored) || is_weak_etag(fresh) {
            if let (Some(stored_lm), Some(fresh_lm)) =
                (info.last_modified.as_deref(), fresh_last_modified)
            {
                if stored_lm != fresh_lm {
                    return ResumeDecision::RestartFromZero;
                }
            } else {
                return ResumeDecision::RestartFromZero;
            }
        }
    } else if let (Some(stored_lm), Some(fresh_lm)) =
        (info.last_modified.as_deref(), fresh_last_modified)
    {
        if stored_lm != fresh_lm {
            return ResumeDecision::RestartFromZero;
        }
    }

    ResumeDecision::ResumeFrom(info.written)
}

/// Reconciles the recorded byte count with the real length of the partial file.
///
/// A longer file is truncated back to the recorded offset before resuming; a
/// shorter file means data was lost, so the download restarts from zero.
pub fn reconcile_partial_len(info: &ResumeInfo, on_disk_len: u64) -> ResumeDecision {
    if let Some(total) = info.total_size {
        if on_disk_len > total {
            return ResumeDecision::Reject("partial file is larger than the resource");
        }
    }
    if on_disk_len < info.written {
        return ResumeDecision::RestartFromZero;
    }
    ResumeDecision::ResumeFrom(info.written)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(etag: Option<&str>, last_modified: Option<&str>, total: Option<u64>, written: u64) -> ResumeInfo {
        ResumeInfo {
            url: "https://example.com/ملف.bin".into(),
            etag: etag.map(str::to_string),
            last_modified: last_modified.map(str::to_string),
            total_size: total,
            written,
        }
    }

    #[test]
    fn weak_etag_detection() {
        assert!(is_weak_etag("W/\"abc\""));
        assert!(is_weak_etag("w/\"abc\""));
        assert!(!is_weak_etag("\"abc\""));
        assert!(!is_weak_etag("  \"W/not-weak\""));
    }

    #[test]
    fn validator_kind_classification() {
        assert_eq!(validator_kind(&info(Some("\"s\""), None, Some(10), 1)), ValidatorKind::StrongEtag);
        assert_eq!(validator_kind(&info(Some("W/\"w\""), None, Some(10), 1)), ValidatorKind::WeakEtag);
        assert_eq!(
            validator_kind(&info(None, Some("Wed, 21 Oct 2015 07:28:00 GMT"), Some(10), 1)),
            ValidatorKind::LastModified
        );
        assert_eq!(validator_kind(&info(None, None, Some(10), 1)), ValidatorKind::None);
    }

    #[test]
    fn if_range_never_emits_weak_etag() {
        assert_eq!(
            if_range_value(&info(Some("\"strong\""), None, Some(10), 1)).as_deref(),
            Some("\"strong\"")
        );
        assert_eq!(
            if_range_value(&info(Some("W/\"weak\""), Some("Wed, 21 Oct 2015 07:28:00 GMT"), Some(10), 1))
                .as_deref(),
            Some("Wed, 21 Oct 2015 07:28:00 GMT")
        );
        assert_eq!(if_range_value(&info(Some("W/\"weak\""), None, Some(10), 1)), None);
        assert_eq!(if_range_value(&info(None, None, Some(10), 1)), None);
    }

    #[test]
    fn resume_allowed_when_strong_etag_and_size_match() {
        let i = info(Some("\"v1\""), None, Some(1024), 512);
        assert_eq!(validate_resume(&i, Some("\"v1\""), None, Some(1024)), ResumeDecision::ResumeFrom(512));
    }

    #[test]
    fn total_size_change_forces_restart() {
        let i = info(Some("\"v1\""), None, Some(1024), 512);
        assert_eq!(
            validate_resume(&i, Some("\"v1\""), None, Some(2048)),
            ResumeDecision::RestartFromZero
        );
    }

    #[test]
    fn strong_etag_change_forces_restart() {
        let i = info(Some("\"v1\""), None, Some(1024), 512);
        assert_eq!(
            validate_resume(&i, Some("\"v2\""), None, Some(1024)),
            ResumeDecision::RestartFromZero
        );
    }

    #[test]
    fn weak_etag_never_authorises_resume() {
        let i = info(Some("W/\"v1\""), None, Some(1024), 512);
        assert_eq!(
            validate_resume(&i, Some("W/\"v1\""), None, Some(1024)),
            ResumeDecision::RestartFromZero
        );
    }

    #[test]
    fn weak_etag_falls_back_to_last_modified() {
        // A weak ETag cannot authorise a range resume, but an identical
        // Last-Modified is a usable validator, so the resume is allowed.
        let i = info(
            Some("W/\"v1\""),
            Some("Wed, 21 Oct 2015 07:28:00 GMT"),
            Some(1024),
            512,
        );
        assert_eq!(
            validate_resume(
                &i,
                Some("W/\"v1\""),
                Some("Wed, 21 Oct 2015 07:28:00 GMT"),
                Some(1024)
            ),
            ResumeDecision::ResumeFrom(512)
        );
        // A different Last-Modified invalidates the resume.
        assert_eq!(
            validate_resume(
                &i,
                Some("W/\"v1\""),
                Some("Thu, 22 Oct 2015 07:28:00 GMT"),
                Some(1024)
            ),
            ResumeDecision::RestartFromZero
        );
    }

    #[test]
    fn missing_validator_on_either_side_forces_restart() {
        let with_validator = info(Some("\"v1\""), None, Some(1024), 512);
        assert_eq!(
            validate_resume(&with_validator, None, None, Some(1024)),
            ResumeDecision::RestartFromZero
        );

        let without_validator = info(None, None, Some(1024), 512);
        assert_eq!(
            validate_resume(&without_validator, Some("\"v1\""), None, Some(1024)),
            ResumeDecision::RestartFromZero
        );
        assert_eq!(
            validate_resume(&without_validator, None, None, Some(1024)),
            ResumeDecision::RestartFromZero
        );
    }

    #[test]
    fn last_modified_only_resume_requires_identical_timestamp() {
        let i = info(None, Some("Wed, 21 Oct 2015 07:28:00 GMT"), Some(1024), 512);
        assert_eq!(
            validate_resume(&i, None, Some("Wed, 21 Oct 2015 07:28:00 GMT"), Some(1024)),
            ResumeDecision::ResumeFrom(512)
        );
        assert_eq!(
            validate_resume(&i, None, Some("Thu, 22 Oct 2015 07:28:00 GMT"), Some(1024)),
            ResumeDecision::RestartFromZero
        );
    }

    #[test]
    fn written_equal_to_total_is_already_complete() {
        let i = info(Some("\"v1\""), None, Some(1024), 1024);
        assert_eq!(
            validate_resume(&i, Some("\"v1\""), None, Some(1024)),
            ResumeDecision::AlreadyComplete
        );
    }

    #[test]
    fn written_beyond_total_is_rejected() {
        let i = info(Some("\"v1\""), None, Some(1024), 4096);
        assert!(matches!(
            validate_resume(&i, Some("\"v1\""), None, Some(1024)),
            ResumeDecision::Reject(_)
        ));
    }

    #[test]
    fn reconcile_truncates_longer_file_and_restarts_shorter_file() {
        let i = info(Some("\"v1\""), None, Some(1024), 512);
        assert_eq!(reconcile_partial_len(&i, 512), ResumeDecision::ResumeFrom(512));
        assert_eq!(reconcile_partial_len(&i, 900), ResumeDecision::ResumeFrom(512));
        assert_eq!(reconcile_partial_len(&i, 100), ResumeDecision::RestartFromZero);
        assert!(matches!(reconcile_partial_len(&i, 4096), ResumeDecision::Reject(_)));
    }

    #[test]
    fn resume_info_round_trips_through_json() {
        let i = info(Some("\"v1\""), None, Some(1024), 512);
        let json = serde_json::to_string(&i).expect("serialize");
        let back: ResumeInfo = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(i, back);
    }
}

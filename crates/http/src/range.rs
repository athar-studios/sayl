//! HTTP range capabilities, `Content-Range` parsing, and range validation.
//!
//! Provides [`RangeSupport`] and verification logic ensuring capability downgrade
//! on server mismatches without assuming support blindly.

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::error::HttpError;

/// Capability classification for origin HTTP range support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum RangeSupport {
    /// Origin responded with 206 Partial Content, a valid `Content-Range` header,
    /// and total length consistent with advertised size. Safe for parallel/resume.
    Supported,

    /// Origin responded with 200 OK or full body, ignoring the `Range` request header.
    /// Range requests must be downgraded to sequential streaming.
    Ignored,

    /// Origin explicitly rejected ranges (e.g., `Accept-Ranges: none`, malformed range,
    /// or 416 Range Not Satisfiable). Only single-stream download allowed.
    Unsupported,

    /// Range support has not yet been probed or verified.
    #[default]
    Unknown,
}

impl RangeSupport {
    /// Returns `true` if multi-connection chunking and range resumption are verified safe.
    #[inline]
    pub const fn is_supported(&self) -> bool {
        matches!(self, Self::Supported)
    }

    /// Returns `true` if resumption or chunking is possible.
    #[inline]
    pub const fn is_resumable(&self) -> bool {
        matches!(self, Self::Supported)
    }

    /// Returns the string representation of the range support status.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Supported => "Supported",
            Self::Ignored => "Ignored",
            Self::Unsupported => "Unsupported",
            Self::Unknown => "Unknown",
        }
    }
}

/// Parsed representation of an RFC 9110 `Content-Range` header.
///
/// Format: `bytes <unit-first>-<unit-last>/<total>` or `bytes */<total>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentRange {
    /// First byte offset of the range slice (inclusive).
    pub start: Option<u64>,
    /// Last byte offset of the range slice (inclusive).
    pub end: Option<u64>,
    /// Total entity body size in bytes, if known.
    pub total: Option<u64>,
}

impl ContentRange {
    /// Parses a `Content-Range` header value string.
    ///
    /// # Examples
    /// ```
    /// use sayl_http::range::ContentRange;
    ///
    /// let cr = ContentRange::parse("bytes 0-499/1234").unwrap();
    /// assert_eq!(cr.start, Some(0));
    /// assert_eq!(cr.end, Some(499));
    /// assert_eq!(cr.total, Some(1234));
    ///
    /// let unsatisfied = ContentRange::parse("bytes */1234").unwrap();
    /// assert_eq!(unsatisfied.start, None);
    /// assert_eq!(unsatisfied.total, Some(1234));
    /// ```
    pub fn parse(value: &str) -> Result<Self, HttpError> {
        let trimmed = value.trim();
        let (unit, rest) = trimmed.split_once(' ').ok_or_else(|| {
            HttpError::invalid_header("Content-Range", "missing space after unit")
        })?;

        if !unit.eq_ignore_ascii_case("bytes") {
            return Err(HttpError::invalid_header(
                "Content-Range",
                format!("unsupported unit '{unit}' (expected 'bytes')"),
            ));
        }

        let (range_part, total_part) = rest
            .split_once('/')
            .ok_or_else(|| HttpError::invalid_header("Content-Range", "missing '/' delimiter"))?;

        let total = if total_part.trim() == "*" {
            None
        } else {
            let t = total_part.trim().parse::<u64>().map_err(|e| {
                HttpError::invalid_header("Content-Range", format!("invalid total length: {e}"))
            })?;
            Some(t)
        };

        let range_part = range_part.trim();
        if range_part == "*" {
            return Ok(Self {
                start: None,
                end: None,
                total,
            });
        }

        let (start_str, end_str) = range_part.split_once('-').ok_or_else(|| {
            HttpError::invalid_header("Content-Range", "missing '-' in byte range")
        })?;

        let start = start_str.trim().parse::<u64>().map_err(|e| {
            HttpError::invalid_header("Content-Range", format!("invalid start offset: {e}"))
        })?;

        let end = end_str.trim().parse::<u64>().map_err(|e| {
            HttpError::invalid_header("Content-Range", format!("invalid end offset: {e}"))
        })?;

        if start > end {
            return Err(HttpError::range(
                format!("invalid Content-Range: start ({start}) > end ({end})"),
                None,
            ));
        }

        if let Some(tot) = total {
            if tot > 0 && end >= tot {
                return Err(HttpError::range(
                    format!("invalid Content-Range: end ({end}) >= total ({tot})"),
                    None,
                ));
            }
        }

        Ok(Self {
            start: Some(start),
            end: Some(end),
            total,
        })
    }
}

/// Formats an RFC 9110 `Range` header value for byte offsets.
pub fn format_byte_range(start: u64, end_inclusive: Option<u64>) -> String {
    match end_inclusive {
        Some(end) => format!("bytes={start}-{end}"),
        None => format!("bytes={start}-"),
    }
}

/// Evaluates origin range capability from a `Range: bytes=0-0` verification subrequest.
///
/// Returns `(RangeSupport, Option<total_size>)`.
///
/// Invariants:
/// - 206 Partial Content + valid Content-Range matching advertised size => [`RangeSupport::Supported`]
/// - 200 OK (server ignored Range header) => [`RangeSupport::Ignored`]
/// - 416 Range Not Satisfiable => [`RangeSupport::Unsupported`] (or handles 0-byte file without error)
/// - Malformed or mismatched Content-Range => downgrades to [`RangeSupport::Unsupported`]
pub fn verify_range_probe_response(
    status: StatusCode,
    content_range_header: Option<&str>,
    advertised_length: Option<u64>,
) -> (RangeSupport, Option<u64>) {
    match status {
        StatusCode::PARTIAL_CONTENT => {
            let Some(raw_cr) = content_range_header else {
                tracing::warn!("origin returned 206 Partial Content without Content-Range header");
                return (RangeSupport::Unsupported, advertised_length);
            };

            let cr = match ContentRange::parse(raw_cr) {
                Ok(cr) => cr,
                Err(err) => {
                    tracing::warn!("failed to parse Content-Range header: {err}");
                    return (RangeSupport::Unsupported, advertised_length);
                }
            };

            // For probe Range: bytes=0-0, verify the response corresponds to byte 0
            if cr.start != Some(0) {
                tracing::warn!("Content-Range start != 0 for probe 0-0 request");
                return (RangeSupport::Unsupported, advertised_length);
            }

            // Verify advertised length consistency if both present
            if let (Some(adv), Some(tot)) = (advertised_length, cr.total) {
                if adv != tot {
                    tracing::warn!(
                        "advertised Content-Length ({adv}) does not match Content-Range total ({tot})"
                    );
                    return (RangeSupport::Unsupported, Some(tot));
                }
            }

            let resolved_len = cr.total.or(advertised_length);
            (RangeSupport::Supported, resolved_len)
        }
        StatusCode::OK => {
            // Origin sent 200 OK, completely ignoring the Range: bytes=0-0 header
            tracing::info!("origin ignored Range header and returned 200 OK");
            (RangeSupport::Ignored, advertised_length)
        }
        StatusCode::RANGE_NOT_SATISFIABLE => {
            // 416: Check if empty file (e.g. Content-Range: bytes */0)
            if let Some(raw_cr) = content_range_header {
                if let Ok(cr) = ContentRange::parse(raw_cr) {
                    if cr.total == Some(0) {
                        return (RangeSupport::Unsupported, Some(0));
                    }
                }
            }
            if advertised_length == Some(0) {
                return (RangeSupport::Unsupported, Some(0));
            }

            tracing::info!("origin returned 416 Range Not Satisfiable");
            (RangeSupport::Unsupported, advertised_length)
        }
        _ => (RangeSupport::Unsupported, advertised_length),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_range_valid_parse() {
        let cr = ContentRange::parse("bytes 0-0/1000").unwrap();
        assert_eq!(cr.start, Some(0));
        assert_eq!(cr.end, Some(0));
        assert_eq!(cr.total, Some(1000));

        let cr2 = ContentRange::parse("BYTES 100-499/*").unwrap();
        assert_eq!(cr2.start, Some(100));
        assert_eq!(cr2.end, Some(499));
        assert_eq!(cr2.total, None);

        let cr3 = ContentRange::parse("bytes */500").unwrap();
        assert_eq!(cr3.start, None);
        assert_eq!(cr3.end, None);
        assert_eq!(cr3.total, Some(500));
    }

    #[test]
    fn test_content_range_invalid_parse() {
        // Invalid unit
        assert!(ContentRange::parse("items 0-10/20").is_err());
        // Missing delimiter
        assert!(ContentRange::parse("bytes 0-10").is_err());
        // Start > End
        assert!(ContentRange::parse("bytes 50-10/100").is_err());
        // End >= Total
        assert!(ContentRange::parse("bytes 0-100/100").is_err());
    }

    #[test]
    fn test_format_byte_range() {
        assert_eq!(format_byte_range(0, Some(0)), "bytes=0-0");
        assert_eq!(format_byte_range(100, Some(499)), "bytes=100-499");
        assert_eq!(format_byte_range(500, None), "bytes=500-");
    }

    #[test]
    fn test_verify_range_supported() {
        let (support, size) = verify_range_probe_response(
            StatusCode::PARTIAL_CONTENT,
            Some("bytes 0-0/2048"),
            Some(2048),
        );
        assert_eq!(support, RangeSupport::Supported);
        assert_eq!(size, Some(2048));
    }

    #[test]
    fn test_verify_range_downgrade_on_mismatch() {
        // Total mismatch: advertised 1000 vs Content-Range 2000
        let (support, size) = verify_range_probe_response(
            StatusCode::PARTIAL_CONTENT,
            Some("bytes 0-0/2000"),
            Some(1000),
        );
        assert_eq!(support, RangeSupport::Unsupported);
        assert_eq!(size, Some(2000));
    }

    #[test]
    fn test_verify_range_ignored_200() {
        let (support, size) = verify_range_probe_response(StatusCode::OK, None, Some(5000));
        assert_eq!(support, RangeSupport::Ignored);
        assert_eq!(size, Some(5000));
    }

    #[test]
    fn test_verify_range_416() {
        let (support, size) = verify_range_probe_response(
            StatusCode::RANGE_NOT_SATISFIABLE,
            Some("bytes */0"),
            Some(0),
        );
        assert_eq!(support, RangeSupport::Unsupported);
        assert_eq!(size, Some(0));
    }
}

//! URL deduplication logic for download tasks.
//!
//! Normalizes URLs for comparison so that minor formatting variations (scheme/host casing,
//! presence/absence of trailing slash when path is empty, fragments, and whitespace)
//! are matched to existing active downloads.
//!
//! Note: Default ports (such as `:80` for HTTP or `:443` for HTTPS) are not stripped or normalized.

use sayl_core::state::DownloadState;
use sayl_storage::TaskRecord;

/// Normalizes a URL for comparison according to SAYL's duplicate detection rules:
/// 1. Trim leading and trailing whitespace.
/// 2. Drop everything from the first `#` (fragment identifier).
/// 3. If it contains `"://"`, lowercase the scheme and authority (authority ends at the first `/`),
///    treat an empty path as `"/"`, and keep path + query byte-for-byte.
/// 4. Otherwise, return the trimmed (and unfragmented) string unchanged.
///
/// Note: Default ports (such as `:80` for HTTP or `:443` for HTTPS) are not normalized or stripped.
pub fn normalize_url_for_compare(raw: &str) -> String {
    let mut s = raw.trim();
    if let Some(idx) = s.find('#') {
        s = &s[..idx];
    }

    let Some(scheme_idx) = s.find("://") else {
        return s.to_string();
    };

    let scheme = &s[..scheme_idx];
    let after_scheme = &s[scheme_idx + 3..];

    let (authority, path_and_query) = match after_scheme.find('/') {
        Some(slash_idx) => {
            let auth = &after_scheme[..slash_idx];
            let pq = &after_scheme[slash_idx..];
            (auth, pq.to_string())
        }
        None => match after_scheme.find('?') {
            Some(q_idx) => {
                let auth = &after_scheme[..q_idx];
                let q = &after_scheme[q_idx..];
                (auth, format!("/{q}"))
            }
            None => (after_scheme, "/".to_string()),
        },
    };

    let scheme_lower = scheme.to_ascii_lowercase();
    let authority_lower = authority.to_ascii_lowercase();
    format!("{scheme_lower}://{authority_lower}{path_and_query}")
}

/// Finds an existing task with the same normalized URL.
///
/// Returns `None` if no duplicate exists or if the candidate task is in `DownloadState::Cancelled`.
pub fn find_duplicate<'a>(records: &'a [TaskRecord], url: &str) -> Option<&'a TaskRecord> {
    let target = normalize_url_for_compare(url);
    records.iter().find(|rec| {
        rec.state != DownloadState::Cancelled && normalize_url_for_compare(&rec.url) == target
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_host_scheme_case_folding() {
        let u1 = "HTTP://EXAMPLE.COM/files/archive.zip";
        let u2 = "http://example.com/files/archive.zip";
        assert_eq!(normalize_url_for_compare(u1), normalize_url_for_compare(u2));
    }

    #[test]
    fn test_fragment_removal() {
        let with_frag = "https://example.com/doc.pdf#page=4";
        let without_frag = "https://example.com/doc.pdf";
        assert_eq!(
            normalize_url_for_compare(with_frag),
            normalize_url_for_compare(without_frag)
        );
    }

    #[test]
    fn test_missing_path_equals_slash() {
        let missing = "http://example.com";
        let with_slash = "http://example.com/";
        assert_eq!(normalize_url_for_compare(missing), "http://example.com/");
        assert_eq!(
            normalize_url_for_compare(missing),
            normalize_url_for_compare(with_slash)
        );
    }

    #[test]
    fn test_different_query_is_not_duplicate() {
        let q1 = "http://example.com/file?version=1";
        let q2 = "http://example.com/file?version=2";
        assert_ne!(normalize_url_for_compare(q1), normalize_url_for_compare(q2));
    }

    #[test]
    fn test_whitespace_trimming() {
        let dirty = "   \t https://example.com/test.zip \n ";
        let clean = "https://example.com/test.zip";
        assert_eq!(
            normalize_url_for_compare(dirty),
            normalize_url_for_compare(clean)
        );
    }

    #[test]
    fn test_url_without_scheme() {
        let raw = "some/local/or/custom/path";
        assert_eq!(normalize_url_for_compare(raw), "some/local/or/custom/path");
    }

    #[test]
    fn test_cancelled_excluded() {
        let mut rec1 = TaskRecord::new(
            "http://example.com/file.zip",
            PathBuf::from("C:\\dl"),
            "file.zip",
        );
        rec1.state = DownloadState::Cancelled;

        let mut rec2 = TaskRecord::new(
            "http://example.com/other.zip",
            PathBuf::from("C:\\dl"),
            "other.zip",
        );
        rec2.state = DownloadState::Downloading;

        let records = vec![rec1, rec2];

        // Matching rec1 (Cancelled) -> should return None
        assert!(find_duplicate(&records, "http://example.com/file.zip").is_none());

        // Matching rec2 (Downloading) -> should return Some(rec2)
        let found = find_duplicate(&records, "http://example.com/other.zip");
        assert!(found.is_some());
        assert_eq!(found.unwrap().filename, "other.zip");
    }
}

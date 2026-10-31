//! Validation of everything the extension sends.
//!
//! The bridge is a trust boundary: a compromised extension can send anything, so
//! URLs are restricted to `http`/`https` without embedded credentials, private
//! networks are refused unless the user opted in, and filenames are sanitised and
//! confined to the download directory.

use std::path::{Component, Path, PathBuf};

use url::Url;

use crate::protocol::BridgeError;

/// Longest accepted URL, in bytes.
pub const MAX_URL_BYTES: usize = 8 * 1024;

/// Longest accepted filename, in bytes.
pub const MAX_FILENAME_BYTES: usize = 255;

/// Validates a URL requested by the extension.
///
/// Rejects non-`http(s)` schemes, embedded credentials, control characters,
/// oversized URLs, and — when `allow_private_hosts` is false — loopback and
/// private-network targets.
pub fn validate_request_url(url: &str, allow_private_hosts: bool) -> Result<Url, BridgeError> {
    if url.len() > MAX_URL_BYTES {
        return Err(BridgeError::InvalidUrl(format!(
            "url is {} bytes, the cap is {MAX_URL_BYTES}",
            url.len()
        )));
    }
    if url.chars().any(|c| c.is_control()) {
        return Err(BridgeError::InvalidUrl(
            "url contains a control character".into(),
        ));
    }

    let parsed = Url::parse(url).map_err(|e| BridgeError::InvalidUrl(e.to_string()))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => {
            return Err(BridgeError::InvalidUrl(format!(
                "scheme '{other}' is not allowed; only http and https"
            )))
        }
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(BridgeError::InvalidUrl(
            "url carries embedded credentials".into(),
        ));
    }
    if parsed.host_str().is_none() {
        return Err(BridgeError::InvalidUrl("url has no host".into()));
    }
    if !allow_private_hosts && sayl_http::redirect::is_private_or_loopback_host(&parsed) {
        return Err(BridgeError::InvalidUrl(
            "private and loopback targets are disabled".into(),
        ));
    }
    Ok(parsed)
}

/// Sanitises a filename suggested by the extension.
///
/// The result never contains a path separator or a reserved device name, is never
/// empty, and never exceeds [`MAX_FILENAME_BYTES`].
pub fn validate_suggested_filename(name: &str) -> Result<String, BridgeError> {
    if name.chars().any(|c| c.is_control()) {
        return Err(BridgeError::InvalidUrl(
            "suggested filename contains a control character".into(),
        ));
    }
    let safe = sayl_http::filename::sanitize_filename(name);
    if safe.is_empty() || safe == "." || safe == ".." {
        return Err(BridgeError::PathEscape);
    }
    if safe.len() > MAX_FILENAME_BYTES {
        return Err(BridgeError::InvalidUrl(format!(
            "suggested filename is {} bytes, the cap is {MAX_FILENAME_BYTES}",
            safe.len()
        )));
    }
    // Defence in depth: even if the sanitizer changed, no component may be a
    // separator, a parent directory, or a root.
    for component in Path::new(&safe).components() {
        match component {
            Component::Normal(_) => {}
            _ => return Err(BridgeError::PathEscape),
        }
    }
    Ok(safe)
}

/// Resolves `name` inside `dir`, refusing anything that is not a direct child.
pub fn resolve_within(dir: &Path, name: &str) -> Result<PathBuf, BridgeError> {
    let safe = validate_suggested_filename(name)?;
    let candidate = dir.join(&safe);
    if !candidate.starts_with(dir) {
        return Err(BridgeError::PathEscape);
    }
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_https() {
        let url = validate_request_url("https://example.com/file.zip", false).unwrap();
        assert_eq!(url.scheme(), "https");
    }

    #[test]
    fn rejects_non_http_schemes() {
        for bad in [
            "ftp://example.com/a.zip",
            "file:///C:/Windows/System32/x.dll",
            "javascript:alert(1)",
        ] {
            let err = validate_request_url(bad, true).unwrap_err();
            assert!(
                matches!(err, BridgeError::InvalidUrl(_)),
                "{bad} gave {err:?}"
            );
        }
    }

    #[test]
    fn rejects_embedded_credentials() {
        let err = validate_request_url("https://user:secret@example.com/a.zip", false).unwrap_err();
        match err {
            BridgeError::InvalidUrl(msg) => assert!(
                !msg.contains("secret"),
                "message leaked the password: {msg}"
            ),
            other => panic!("expected InvalidUrl, got {other:?}"),
        }
    }

    #[test]
    fn rejects_control_characters_and_oversized_urls() {
        assert!(validate_request_url("https://example.com/a\r\nHost: evil", false).is_err());
        let long = format!("https://example.com/{}", "a".repeat(MAX_URL_BYTES));
        assert!(validate_request_url(&long, false).is_err());
    }

    #[test]
    fn private_targets_are_blocked_only_when_asked() {
        let local = "http://127.0.0.1:8080/a.bin";
        assert!(validate_request_url(local, false).is_err());
        assert!(validate_request_url(local, true).is_ok());
        let lan = "http://192.168.1.10/a.bin";
        assert!(validate_request_url(lan, false).is_err());
        assert!(validate_request_url(lan, true).is_ok());
    }

    #[test]
    fn traversal_filenames_are_neutralised_or_rejected() {
        let cases = [
            "../../evil.exe",
            "..\\..\\evil.exe",
            "C:\\Windows\\evil.dll",
            "..",
        ];
        for case in cases {
            match validate_suggested_filename(case) {
                Ok(safe) => {
                    assert!(!safe.contains(".."), "{case} produced {safe}");
                    assert!(
                        !safe.contains('/') && !safe.contains('\\'),
                        "{case} produced {safe}"
                    );
                }
                Err(BridgeError::PathEscape) => {}
                Err(other) => panic!("unexpected error for {case}: {other:?}"),
            }
        }
    }

    #[test]
    fn resolve_within_refuses_to_escape() {
        let dir = Path::new("C:\\Downloads");
        // Either the name is refused outright, or it is neutralised into a direct
        // child of `dir`; both are acceptable, escaping never is.
        match resolve_within(dir, "..\\..\\Windows\\evil.dll") {
            Err(BridgeError::PathEscape) => {}
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(path) => {
                assert!(path.starts_with(dir), "escaped the directory: {path:?}");
                assert_eq!(path.parent(), Some(dir), "not a direct child: {path:?}");
            }
        }
        // A name that the sanitizer neutralises into a plain child is accepted and stays inside.
        let ok = resolve_within(dir, "sub/evil.exe").unwrap();
        assert!(ok.starts_with(dir));
        assert!(!ok.to_string_lossy().contains(".."));
    }

    #[test]
    fn normal_arabic_filename_is_preserved() {
        let safe = validate_suggested_filename("تقرير-سنوي 2026.pdf").unwrap();
        assert!(safe.contains("تقرير"), "got {safe}");
        assert!(safe.ends_with(".pdf"), "got {safe}");
    }
}

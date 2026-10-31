//! Win32 clipboard reader and download URL extractor.

#[cfg(windows)]
use windows_sys::Win32::Foundation::HGLOBAL;
#[cfg(windows)]
use windows_sys::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};
#[cfg(windows)]
use windows_sys::Win32::System::Memory::{GlobalLock, GlobalUnlock};

#[cfg(windows)]
const CF_UNICODETEXT: u32 = 13;

#[cfg(windows)]
struct ClipboardGuard;

#[cfg(windows)]
impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
    }
}

#[cfg(windows)]
struct GlobalLockGuard(HGLOBAL);

#[cfg(windows)]
impl Drop for GlobalLockGuard {
    fn drop(&mut self) {
        unsafe {
            GlobalUnlock(self.0);
        }
    }
}

/// Reads the current clipboard text as UTF-8 string on Windows.
/// Never panics; returns `None` on any failure or if the clipboard does not contain text.
#[cfg(windows)]
pub fn read_clipboard_text() -> Option<String> {
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let _clipboard_guard = ClipboardGuard;

        let handle = GetClipboardData(CF_UNICODETEXT);
        if handle.is_null() {
            return None;
        }

        let ptr = GlobalLock(handle);
        if ptr.is_null() {
            return None;
        }
        let _lock_guard = GlobalLockGuard(handle);

        let wide = ptr as *const u16;
        let mut len = 0usize;
        // Cap scanning to 1 million characters to guard against corrupted or huge buffers
        while *wide.add(len) != 0 {
            len += 1;
            if len > 1_000_000 {
                break;
            }
        }

        let slice = std::slice::from_raw_parts(wide, len);
        String::from_utf16(slice).ok()
    }
}

/// Non-Windows fallback for reading clipboard text.
#[cfg(not(windows))]
pub fn read_clipboard_text() -> Option<String> {
    None
}

/// Pure state tracker for clipboard monitoring.
///
/// Ensures:
/// - When monitoring is disabled, no seeding or tracking occurs.
/// - When first enabled (or on startup with enabled=true), seeds with the current text
///   WITHOUT emitting a URL.
/// - Once seeded, any subsequent change to a valid download URL returns `Some(url)`.
/// - Never emits the same text twice in a row.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ClipboardMonitorState {
    last_seen: Option<String>,
    seeded: bool,
}

impl ClipboardMonitorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Evaluates one tick of clipboard monitoring.
    ///
    /// If `enabled` is false, returns `None` and does not mutate or seed state.
    /// If `enabled` is true and not yet seeded, seeds `last_seen` with `current` (if any),
    /// marks `seeded = true`, and returns `None`.
    /// Once seeded, if `current` has changed, updates `last_seen` and returns `extract_download_url(text)`.
    pub fn tick(&mut self, enabled: bool, current: Option<String>) -> Option<String> {
        if !enabled {
            return None;
        }

        if !self.seeded {
            self.seeded = true;
            self.last_seen = current;
            return None;
        }

        match current {
            Some(text) => {
                if self.last_seen.as_deref() != Some(&text) {
                    let url = extract_download_url(&text);
                    self.last_seen = Some(text);
                    url
                } else {
                    None
                }
            }
            None => None,
        }
    }

    pub fn is_seeded(&self) -> bool {
        self.seeded
    }

    pub fn last_seen(&self) -> Option<&str> {
        self.last_seen.as_deref()
    }
}

/// Pure helper to extract a single download URL from text.
///
/// Rules:
/// - Leading and trailing whitespace is trimmed.
/// - Length must not exceed 8192 characters and must not be empty.
/// - Must not contain inner whitespace or newlines (single token, no spaces/CR/LF).
/// - Must have a valid "http" or "https" scheme and a non-empty host.
pub fn extract_download_url(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.len() > 8192 {
        return None;
    }

    if trimmed
        .chars()
        .any(|c| c.is_whitespace() || c == '\r' || c == '\n')
    {
        return None;
    }

    let parsed = url::Url::parse(trimmed).ok()?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return None;
    }

    if parsed.host_str().unwrap_or("").is_empty() {
        return None;
    }

    Some(trimmed.to_string())
}

/// Extracts all valid download URLs from clipboard text.
/// Supports multiple URLs separated by whitespace or newlines.
/// Returns a vector of valid http/https/ftp URLs.
pub fn extract_download_urls(text: &str) -> Vec<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    // Split by whitespace (spaces, tabs, newlines)
    trimmed
        .split_whitespace()
        .filter_map(|token| {
            let token_trimmed = token.trim();
            if token_trimmed.is_empty() || token_trimmed.len() > 8192 {
                return None;
            }

            // Check for valid URL scheme
            let parsed = url::Url::parse(token_trimmed).ok()?;
            let scheme = parsed.scheme();
            if scheme != "http" && scheme != "https" && scheme != "ftp" {
                return None;
            }

            if parsed.host_str().unwrap_or("").is_empty() {
                return None;
            }

            Some(token_trimmed.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_download_url_valid_http_and_https() {
        assert_eq!(
            extract_download_url("http://example.com/file.zip"),
            Some("http://example.com/file.zip".to_string())
        );
        assert_eq!(
            extract_download_url(
                "https://downloads.example.org/v1/app.tar.gz?token=abc&dl=1#checksum"
            ),
            Some("https://downloads.example.org/v1/app.tar.gz?token=abc&dl=1#checksum".to_string())
        );
        assert_eq!(
            extract_download_url("https://example.com"),
            Some("https://example.com".to_string())
        );
    }

    #[test]
    fn test_extract_download_url_trims_whitespace() {
        assert_eq!(
            extract_download_url("  \t https://example.com/archive.zip \r\n  "),
            Some("https://example.com/archive.zip".to_string())
        );
    }

    #[test]
    fn test_extract_download_url_rejects_empty_and_spaces() {
        assert_eq!(extract_download_url(""), None);
        assert_eq!(extract_download_url("   \t\r\n"), None);
        assert_eq!(
            extract_download_url("https://example.com/path with spaces/file.zip"),
            None
        );
        assert_eq!(
            extract_download_url("https://example.com/1 https://example.com/2"),
            None
        );
    }

    #[test]
    fn test_extract_download_url_rejects_non_http_schemes() {
        assert_eq!(extract_download_url("ftp://ftp.example.com/file.bin"), None);
        assert_eq!(extract_download_url("javascript:alert(1)"), None);
        assert_eq!(extract_download_url("javascript:void(0)"), None);
        assert_eq!(extract_download_url("file:///C:/path/to/file.iso"), None);
        assert_eq!(
            extract_download_url("data:text/plain;base64,SGVsbG8="),
            None
        );
        assert_eq!(extract_download_url("mailto:user@example.com"), None);
    }

    #[test]
    fn test_extract_download_url_rejects_multiline() {
        assert_eq!(
            extract_download_url("https://example.com/1\nhttps://example.com/2"),
            None
        );
        assert_eq!(
            extract_download_url("https://example.com/1\r\nhttps://example.com/2"),
            None
        );
        assert_eq!(
            extract_download_url("prefix\nhttps://example.com/file.zip"),
            None
        );
    }

    #[test]
    fn test_extract_download_url_length_bounds() {
        let base = "https://example.com/";
        let fill_len = 8192 - base.len();
        let valid_max = format!("{}{}", base, "a".repeat(fill_len));
        assert_eq!(valid_max.len(), 8192);
        assert_eq!(extract_download_url(&valid_max), Some(valid_max.clone()));

        let too_long = format!("{}b", valid_max);
        assert_eq!(too_long.len(), 8193);
        assert_eq!(extract_download_url(&too_long), None);
    }

    #[test]
    fn test_extract_download_url_rejects_no_host() {
        assert_eq!(extract_download_url("http://"), None);
        assert_eq!(extract_download_url("https://"), None);
        assert_eq!(extract_download_url("http:///"), None);
    }

    #[test]
    fn test_clipboard_monitor_state_disabled_does_not_seed_or_emit() {
        let mut state = ClipboardMonitorState::new();
        assert!(!state.is_seeded());
        assert_eq!(state.last_seen(), None);

        // While disabled, passing a URL does nothing and does not mark seeded
        let res = state.tick(false, Some("https://example.com/file.zip".to_string()));
        assert_eq!(res, None);
        assert!(!state.is_seeded());
        assert_eq!(state.last_seen(), None);
    }

    #[test]
    fn test_clipboard_monitor_state_lazy_seeding_does_not_emit_on_first_tick() {
        let mut state = ClipboardMonitorState::new();

        // 1. Initial tick with enabled=true: seeds but does NOT emit
        let res = state.tick(true, Some("https://example.com/initial.zip".to_string()));
        assert_eq!(res, None);
        assert!(state.is_seeded());
        assert_eq!(state.last_seen(), Some("https://example.com/initial.zip"));

        // 2. Same content on next tick: does not emit
        let res2 = state.tick(true, Some("https://example.com/initial.zip".to_string()));
        assert_eq!(res2, None);

        // 3. Changed content with valid URL: emits
        let res3 = state.tick(true, Some("https://example.com/new.zip".to_string()));
        assert_eq!(res3, Some("https://example.com/new.zip".to_string()));
        assert_eq!(state.last_seen(), Some("https://example.com/new.zip"));

        // 4. Repeated content: does not emit
        let res4 = state.tick(true, Some("https://example.com/new.zip".to_string()));
        assert_eq!(res4, None);

        // 5. Changed to non-URL text: updates last_seen but returns None
        let res5 = state.tick(true, Some("hello world".to_string()));
        assert_eq!(res5, None);
        assert_eq!(state.last_seen(), Some("hello world"));
    }

    #[test]
    fn test_clipboard_monitor_state_delayed_enable_seeds_lazily() {
        let mut state = ClipboardMonitorState::new();

        // Disabled on launch
        assert_eq!(state.tick(false, None), None);
        assert!(!state.is_seeded());

        // Later enabled when user copies something
        let res = state.tick(true, Some("https://example.com/seeded.zip".to_string()));
        assert_eq!(res, None); // Seeded, not emitted!
        assert!(state.is_seeded());

        // Subsequent copy emits
        let res2 = state.tick(true, Some("https://example.com/next.zip".to_string()));
        assert_eq!(res2, Some("https://example.com/next.zip".to_string()));
    }

    #[test]
    fn test_extract_download_urls_single_with_surrounding_whitespace() {
        assert_eq!(
            extract_download_urls("  \t\r\n https://example.com/archive.zip \n\t  "),
            vec!["https://example.com/archive.zip".to_string()]
        );
    }

    #[test]
    fn test_extract_download_urls_one_per_line_preserves_order() {
        let text = concat!(
            "https://example.com/a.zip\n",
            "http://example.com/b.iso\r\n",
            "ftp://ftp.example.com/c.bin"
        );
        assert_eq!(
            extract_download_urls(text),
            vec![
                "https://example.com/a.zip".to_string(),
                "http://example.com/b.iso".to_string(),
                "ftp://ftp.example.com/c.bin".to_string(),
            ]
        );
    }

    #[test]
    fn test_extract_download_urls_skips_junk_lines() {
        let text = concat!(
            "hello world\n",
            "https://example.com/a.zip\n",
            "not a url\n",
            "\thttps://example.com/b.zip  "
        );
        assert_eq!(
            extract_download_urls(text),
            vec![
                "https://example.com/a.zip".to_string(),
                "https://example.com/b.zip".to_string(),
            ]
        );
    }

    #[test]
    fn test_extract_download_urls_scheme_filtering() {
        let text = [
            "ftp://ftp.example.com/file.bin",
            "https://example.com/secure.zip",
            "http://example.com/plain.zip",
            "javascript:alert(1)",
            "javascript:void(0)",
            "data:text/plain;base64,SGVsbG8=",
            "file:///C:/Windows/file.iso",
            "mailto:user@example.com",
        ]
        .join("\n");

        assert_eq!(
            extract_download_urls(&text),
            vec![
                "ftp://ftp.example.com/file.bin".to_string(),
                "https://example.com/secure.zip".to_string(),
                "http://example.com/plain.zip".to_string(),
            ]
        );
    }

    /// KNOWN GAP (documented, not silently accepted):
    /// `extract_download_urls` does NOT de-duplicate — it returns every matching
    /// token in order. This file is test-only scope, so the behavior is pinned
    /// here as-is. The user-facing paste path still de-duplicates because
    /// `App.svelte` re-runs the result through `extractUrls()` in
    /// `src/lib/clipboardPaste.js`, which de-dupes (see clipboardPaste.test.js).
    #[test]
    fn test_extract_download_urls_duplicates_are_preserved_by_rust_layer() {
        let text = concat!(
            "https://example.com/a.zip\n",
            "https://example.com/b.zip\n",
            "https://example.com/a.zip"
        );
        assert_eq!(
            extract_download_urls(text),
            vec![
                "https://example.com/a.zip".to_string(),
                "https://example.com/b.zip".to_string(),
                "https://example.com/a.zip".to_string(),
            ]
        );
    }

    #[test]
    fn test_extract_download_urls_empty_and_whitespace_only() {
        assert!(extract_download_urls("").is_empty());
        assert!(extract_download_urls("   \t\r\n\n ").is_empty());
    }
}

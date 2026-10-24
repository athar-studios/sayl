//! Safe filename derivation, RFC 5987/6266 parsing, sanitization, and Windows reserved name protection.
//!
//! Enforces:
//! - Extraction from `Content-Disposition` (`filename*=UTF-8''...` then `filename="..."`).
//! - Fallback to URL path segments, then default fallback.
//! - Neutralization of directory traversal (`..`), path separators (`/`, `\`).
//! - Removal of control characters and Windows-reserved characters (`<>:"/\|?*`).
//! - Windows reserved device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9`) neutralized with extension preserved.
//! - Maximum 255 character length cap while preserving extensions.
//! - Unicode NFC normalization with full Arabic script preservation.

use percent_encoding::percent_decode_str;
use unicode_normalization::UnicodeNormalization;
use url::Url;

/// Maximum filename length permitted under Windows NTFS / FAT32 file systems.
pub const MAX_FILENAME_LENGTH: usize = 255;

/// Default fallback filename when neither header nor URL provides a valid name.
pub const DEFAULT_FALLBACK_FILENAME: &str = "download.bin";

/// Windows reserved device names (case-insensitive).
const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Characters disallowed in Windows filenames:
/// - Windows reserved characters: `< > : " / \ | ? *`
/// - ASCII control characters: `0x00..=0x1F`, `0x7F`
/// - BiDi control characters: U+202A–U+202E, U+2066–U+2069, U+200E, U+200F, U+061C
/// - Zero-width formatters: U+200B–U+200D, U+FEFF
fn is_disallowed_char(c: char) -> bool {
    matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
        || (c as u32) <= 0x1F
        || (c as u32) == 0x7F
        || matches!(
            c,
            '\u{202A}'..='\u{202E}'
                | '\u{2066}'..='\u{2069}'
                | '\u{200E}'
                | '\u{200F}'
                | '\u{200B}'..='\u{200D}'
                | '\u{FEFF}'
                | '\u{061C}'
        )
}

/// Derives a safe, sanitized filename from `Content-Disposition`, URL, or fallback.
pub fn derive_filename(
    content_disposition: Option<&str>,
    url: &Url,
    fallback: Option<&str>,
) -> String {
    // 1. Try parsing Content-Disposition header
    if let Some(cd) = content_disposition {
        if let Some(name) = parse_content_disposition(cd) {
            let sanitized = sanitize_filename(&name);
            if !sanitized.is_empty() && sanitized != DEFAULT_FALLBACK_FILENAME {
                return sanitized;
            }
        }
    }

    // 2. Try extracting from URL path
    if let Some(name) = extract_filename_from_url(url) {
        let sanitized = sanitize_filename(&name);
        if !sanitized.is_empty() && sanitized != DEFAULT_FALLBACK_FILENAME {
            return sanitized;
        }
    }

    // 3. Fallback
    let fb = fallback.unwrap_or(DEFAULT_FALLBACK_FILENAME);
    sanitize_filename(fb)
}

/// Parses `Content-Disposition` according to RFC 6266 and RFC 5987.
///
/// Prioritizes `filename*` (extended parameter) over standard `filename`.
pub fn parse_content_disposition(header: &str) -> Option<String> {
    let mut standard_filename = None;
    let mut extended_filename = None;

    // Direct search for parameters
    for part in header.split(';') {
        let part = part.trim();
        if let Some(val) = part.strip_prefix("filename*=") {
            let val = val.trim().trim_matches('"');
            if let Some(decoded) = parse_rfc5987(val) {
                extended_filename = Some(decoded);
            }
        } else if let Some(val) = part.strip_prefix("filename=") {
            let val = val.trim();
            let unquoted = if val.starts_with('"') && val.ends_with('"') && val.len() >= 2 {
                &val[1..val.len() - 1]
            } else {
                val
            };
            standard_filename = Some(unquoted.to_string());
        }
    }

    extended_filename.or(standard_filename)
}

/// Parses an RFC 5987 encoded string such as `UTF-8''%D9%85%D9%84%D9%81.pdf` or `utf-8'en'%D9%85...`.
fn parse_rfc5987(val: &str) -> Option<String> {
    let parts: Vec<&str> = val.splitn(3, '\'').collect();
    if parts.len() == 3 {
        let charset = parts[0];
        let encoded_value = parts[2];
        if charset.eq_ignore_ascii_case("utf-8") {
            let decoded = percent_decode_str(encoded_value).decode_utf8().ok()?;
            return Some(decoded.into_owned());
        }
    } else {
        // Some servers omit the single quotes and just percent-encode
        if let Ok(decoded) = percent_decode_str(val).decode_utf8() {
            return Some(decoded.into_owned());
        }
    }
    None
}

/// Extracts the last non-empty path segment from a URL as a filename candidate.
pub fn extract_filename_from_url(url: &Url) -> Option<String> {
    let mut segments = url.path_segments()?;
    let last = segments.rfind(|s| !s.is_empty())?;

    let decoded = percent_decode_str(last).decode_utf8().ok()?;
    let name = decoded.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Sanitizes a raw filename string to make it safe for Windows filesystem storage:
/// - Strips path traversal (`..`) and separators (`/`, `\`).
/// - Replaces invalid Windows chars (`<>:"/\|?*`) and control chars with `_`.
/// - NFC normalizes Unicode text.
/// - Neutralizes Windows reserved device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9`).
/// - Enforces maximum length of 255 characters while preserving file extension.
/// - Strips trailing dots and spaces.
pub fn sanitize_filename(raw: &str) -> String {
    // 1. NFC normalization
    let normalized: String = raw.nfc().collect();

    // 2. Remove directory components: take only the final segment if slashes exist
    let without_path = normalized
        .replace('\\', "/")
        .split('/')
        .rfind(|s| !s.is_empty() && *s != "." && *s != "..")
        .unwrap_or("")
        .to_string();

    if without_path.is_empty() {
        return DEFAULT_FALLBACK_FILENAME.to_string();
    }

    // 3. Replace disallowed chars with underscore
    let sanitized_chars: String = without_path
        .chars()
        .map(|c| if is_disallowed_char(c) { '_' } else { c })
        .collect();

    // 4. Strip leading and trailing dots and spaces (Windows does not allow trailing dot/space)
    let trimmed = sanitized_chars.trim_matches(|c| c == ' ' || c == '.');
    if trimmed.is_empty() {
        return DEFAULT_FALLBACK_FILENAME.to_string();
    }

    // 5. Check and neutralize Windows reserved device names
    let neutralized = neutralize_reserved_device_names(trimmed);

    // 6. Cap length at 255 while preserving extension
    truncate_preserving_extension(&neutralized, MAX_FILENAME_LENGTH)
}

/// Checks if the base stem of a filename is a Windows reserved name, and prefixes `_` if so.
///
/// Normalizes the stem before comparison: trims whitespace and trailing dots from the stem
/// and collapses repeated spaces, so `CON .txt`, `NUL.`, `aux `, and `LPT1 .log` are all neutralized.
fn neutralize_reserved_device_names(name: &str) -> String {
    let base_stem = match name.find('.') {
        Some(first_dot) => &name[..first_dot],
        None => name,
    };
    let collapsed = base_stem.split_whitespace().collect::<Vec<_>>().join(" ");
    let normalized_stem = collapsed
        .trim_matches(|c| c == ' ' || c == '.')
        .to_uppercase();

    let is_reserved = WINDOWS_RESERVED_NAMES
        .iter()
        .any(|&res| normalized_stem == res);

    if is_reserved {
        format!("_{name}")
    } else {
        name.to_string()
    }
}

/// Splits a filename into `(stem, Option<extension>)`.
fn split_stem_extension(name: &str) -> (&str, Option<&str>) {
    match name.rfind('.') {
        Some(dot_idx) if dot_idx > 0 => {
            let stem = &name[..dot_idx];
            let ext = &name[dot_idx + 1..];
            (stem, Some(ext))
        }
        _ => (name, None),
    }
}

/// Truncates a filename to at most `max_chars` UTF-8 characters, preserving the file extension.
fn truncate_preserving_extension(name: &str, max_chars: usize) -> String {
    let char_count = name.chars().count();
    if char_count <= max_chars {
        return name.to_string();
    }

    let (stem, ext_opt) = split_stem_extension(name);
    if let Some(ext) = ext_opt {
        let ext_len = ext.chars().count();
        // If extension alone takes too much space, truncate full string
        if ext_len + 2 >= max_chars {
            return name.chars().take(max_chars).collect();
        }

        let max_stem_len = max_chars - 1 - ext_len; // -1 for dot
        let truncated_stem: String = stem.chars().take(max_stem_len).collect();
        let trimmed_stem = truncated_stem.trim_end_matches([' ', '.']);
        format!("{trimmed_stem}.{ext}")
    } else {
        let truncated: String = name.chars().take(max_chars).collect();
        truncated.trim_end_matches([' ', '.']).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rfc5987_arabic_filename() {
        let header =
            "attachment; filename*=UTF-8''%D8%AA%D9%82%D8%B1%D9%8A%D8%B1_%D8%B3%D9%8A%D9%84.pdf";
        let parsed = parse_content_disposition(header).unwrap();
        assert_eq!(parsed, "تقرير_سيل.pdf");

        let url = Url::parse("https://example.com/dl").unwrap();
        let filename = derive_filename(Some(header), &url, None);
        assert_eq!(filename, "تقرير_سيل.pdf");
    }

    #[test]
    fn test_standard_quoted_filename() {
        let header = r#"attachment; filename="setup_v1.0.exe""#;
        let parsed = parse_content_disposition(header).unwrap();
        assert_eq!(parsed, "setup_v1.0.exe");
    }

    #[test]
    fn test_extended_filename_precedence() {
        let header = r#"attachment; filename="fallback.txt"; filename*=UTF-8''primary.txt"#;
        let parsed = parse_content_disposition(header).unwrap();
        assert_eq!(parsed, "primary.txt");
    }

    #[test]
    fn test_path_traversal_neutralization() {
        assert_eq!(sanitize_filename("../../evil.exe"), "evil.exe");
        assert_eq!(sanitize_filename("..\\..\\evil.exe"), "evil.exe");
        assert_eq!(sanitize_filename("/var/log/syslog.txt"), "syslog.txt");
        assert_eq!(
            sanitize_filename("C:\\Windows\\System32\\calc.exe"),
            "calc.exe"
        );
    }

    #[test]
    fn test_windows_reserved_device_names() {
        assert_eq!(sanitize_filename("CON.txt"), "_CON.txt");
        assert_eq!(sanitize_filename("con.tar.gz"), "_con.tar.gz");
        assert_eq!(sanitize_filename("NUL"), "_NUL");
        assert_eq!(sanitize_filename("aux.zip"), "_aux.zip");
        assert_eq!(sanitize_filename("COM1.dat"), "_COM1.dat");
        assert_eq!(sanitize_filename("lpt9.log"), "_lpt9.log");
        assert_eq!(sanitize_filename("PRN"), "_PRN");

        // F3: Stem variants with trailing spaces and dots
        assert_eq!(sanitize_filename("CON .txt"), "_CON .txt");
        assert_eq!(sanitize_filename("CON   .txt"), "_CON   .txt");
        assert_eq!(sanitize_filename("NUL."), "_NUL");
        assert_eq!(sanitize_filename("aux "), "_aux");
        assert_eq!(sanitize_filename("LPT1 .log"), "_LPT1 .log");

        // Normal files with "con" in substring should NOT be affected
        assert_eq!(sanitize_filename("contact.txt"), "contact.txt");
        assert_eq!(sanitize_filename("icon.png"), "icon.png");
    }

    #[test]
    fn test_bidi_and_zero_width_characters_disallowed() {
        // F4: RLO extension spoofing neutralization
        let spoofed = "invoice\u{202E}txt.exe";
        assert_eq!(sanitize_filename(spoofed), "invoice_txt.exe");

        // Test each bidi control character range: U+202A-U+202E
        assert_eq!(sanitize_filename("a\u{202A}b.txt"), "a_b.txt"); // LRE
        assert_eq!(sanitize_filename("a\u{202B}b.txt"), "a_b.txt"); // RLE
        assert_eq!(sanitize_filename("a\u{202C}b.txt"), "a_b.txt"); // PDF
        assert_eq!(sanitize_filename("a\u{202D}b.txt"), "a_b.txt"); // LRO
        assert_eq!(sanitize_filename("a\u{202E}b.txt"), "a_b.txt"); // RLO

        // Test isolates range: U+2066-U+2069
        assert_eq!(sanitize_filename("a\u{2066}b.txt"), "a_b.txt"); // LRI
        assert_eq!(sanitize_filename("a\u{2067}b.txt"), "a_b.txt"); // RLI
        assert_eq!(sanitize_filename("a\u{2068}b.txt"), "a_b.txt"); // FSI
        assert_eq!(sanitize_filename("a\u{2069}b.txt"), "a_b.txt"); // PDI

        // Test directional marks: U+200E, U+200F, U+061C
        assert_eq!(sanitize_filename("a\u{200E}b.txt"), "a_b.txt"); // LRM
        assert_eq!(sanitize_filename("a\u{200F}b.txt"), "a_b.txt"); // RLM
        assert_eq!(sanitize_filename("a\u{061C}b.txt"), "a_b.txt"); // ALM

        // Test zero-width formatters: U+200B-U+200D, U+FEFF
        assert_eq!(sanitize_filename("a\u{200B}b.txt"), "a_b.txt"); // ZWSP
        assert_eq!(sanitize_filename("a\u{200C}b.txt"), "a_b.txt"); // ZWNJ
        assert_eq!(sanitize_filename("a\u{200D}b.txt"), "a_b.txt"); // ZWJ
        assert_eq!(sanitize_filename("a\u{FEFF}b.txt"), "a_b.txt"); // BOM / ZWNBSP
    }

    #[test]
    fn test_reserved_characters_replaced() {
        assert_eq!(
            sanitize_filename(r#"file:name*with?bad<chars>|"#),
            "file_name_with_bad_chars__"
        );
    }

    #[test]
    fn test_length_capping_with_extension_preserved() {
        let long_stem = "a".repeat(300);
        let raw = format!("{long_stem}.tar.gz");
        let sanitized = sanitize_filename(&raw);

        assert_eq!(sanitized.chars().count(), 255);
        assert!(sanitized.ends_with(".gz"));
    }

    #[test]
    fn test_arabic_unicode_preservation_and_nfc() {
        let arabic_name = "ملف_تجريبي_سيل_2026.docx";
        let sanitized = sanitize_filename(arabic_name);
        assert_eq!(sanitized, arabic_name);

        // F4: Ensure Arabic letters, diacritics/tashkeel, and Eastern Arabic digits remain untouched
        let full_arabic = "تَقْرِيرُ_المَبِيعَاتِ_١٤٤٥_سَيْل.pdf";
        assert_eq!(sanitize_filename(full_arabic), full_arabic);
    }

    #[test]
    fn test_empty_or_dots_fallback() {
        assert_eq!(sanitize_filename("..."), DEFAULT_FALLBACK_FILENAME);
        assert_eq!(sanitize_filename("   "), DEFAULT_FALLBACK_FILENAME);
        assert_eq!(sanitize_filename(""), DEFAULT_FALLBACK_FILENAME);
    }

    #[test]
    fn test_derive_from_url() {
        let url =
            Url::parse("https://cdn.example.com/downloads/v2.1/archive.zip?token=secret").unwrap();
        let name = derive_filename(None, &url, None);
        assert_eq!(name, "archive.zip");
    }
}

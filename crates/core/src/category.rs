//! File category taxonomy and IDM-style per-category destination folders.
//!
//! Mirrors the frontend tables in `apps/desktop/src/lib/classify.js`
//! (`EXT_TO_CATEGORY` and `MIME_PATTERNS`): every extension or MIME type added
//! on one side must be added on the other, otherwise the folder shown by the
//! pre-download window and the folder used by the engine can disagree.
//!
//! Categories are resolved in the same priority order as the frontend:
//! 1. extension match,
//! 2. MIME fallback when the extension is absent or unknown,
//! 3. [`FileCategory::Other`].

use std::path::{Path, PathBuf};

/// Extension to category table, ported from `EXT_TO_CATEGORY` in `classify.js`.
///
/// Keep this list and the JavaScript table in sync: the frontend classifies for
/// display and filtering, this table classifies for the on-disk folder.
pub const EXTENSION_TABLE: &[(&str, FileCategory)] = &[
    // Video
    ("mp4", FileCategory::Video),
    ("mkv", FileCategory::Video),
    ("avi", FileCategory::Video),
    ("mov", FileCategory::Video),
    ("webm", FileCategory::Video),
    ("flv", FileCategory::Video),
    ("wmv", FileCategory::Video),
    ("m4v", FileCategory::Video),
    ("3gp", FileCategory::Video),
    ("ts", FileCategory::Video),
    ("mpg", FileCategory::Video),
    ("mpeg", FileCategory::Video),
    ("ogv", FileCategory::Video),
    // Audio
    ("mp3", FileCategory::Audio),
    ("wav", FileCategory::Audio),
    ("flac", FileCategory::Audio),
    ("aac", FileCategory::Audio),
    ("ogg", FileCategory::Audio),
    ("m4a", FileCategory::Audio),
    ("wma", FileCategory::Audio),
    ("opus", FileCategory::Audio),
    ("alac", FileCategory::Audio),
    ("aiff", FileCategory::Audio),
    ("mid", FileCategory::Audio),
    ("midi", FileCategory::Audio),
    // Documents
    ("pdf", FileCategory::Documents),
    ("doc", FileCategory::Documents),
    ("docx", FileCategory::Documents),
    ("xls", FileCategory::Documents),
    ("xlsx", FileCategory::Documents),
    ("ppt", FileCategory::Documents),
    ("pptx", FileCategory::Documents),
    ("txt", FileCategory::Documents),
    ("md", FileCategory::Documents),
    ("csv", FileCategory::Documents),
    ("rtf", FileCategory::Documents),
    ("odt", FileCategory::Documents),
    ("ods", FileCategory::Documents),
    ("odp", FileCategory::Documents),
    ("epub", FileCategory::Documents),
    // Programs
    ("exe", FileCategory::Programs),
    ("msi", FileCategory::Programs),
    ("apk", FileCategory::Programs),
    ("appimage", FileCategory::Programs),
    ("deb", FileCategory::Programs),
    ("rpm", FileCategory::Programs),
    ("pkg", FileCategory::Programs),
    ("jar", FileCategory::Programs),
    ("bat", FileCategory::Programs),
    ("cmd", FileCategory::Programs),
    ("com", FileCategory::Programs),
    // Compressed / archives
    ("zip", FileCategory::Compressed),
    ("rar", FileCategory::Compressed),
    ("7z", FileCategory::Compressed),
    ("tar", FileCategory::Compressed),
    ("gz", FileCategory::Compressed),
    ("bz2", FileCategory::Compressed),
    ("xz", FileCategory::Compressed),
    ("tgz", FileCategory::Compressed),
    ("iso", FileCategory::Compressed),
    ("dmg", FileCategory::Compressed),
    ("cab", FileCategory::Compressed),
    ("z", FileCategory::Compressed),
    ("lz", FileCategory::Compressed),
    ("lzma", FileCategory::Compressed),
];

/// MIME fallback table, ported from `MIME_PATTERNS` in `classify.js`.
///
/// A pattern ending with `/` matches a MIME family prefix (`video/`,
/// `audio/`); every other pattern must match the whole MIME type. Prefix rows
/// come first so they win over the exact rows, exactly as on the frontend.
pub const MIME_TABLE: &[(&str, FileCategory)] = &[
    ("video/", FileCategory::Video),
    ("audio/", FileCategory::Audio),
    ("application/pdf", FileCategory::Documents),
    ("application/msword", FileCategory::Documents),
    (
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        FileCategory::Documents,
    ),
    ("application/vnd.ms-excel", FileCategory::Documents),
    (
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        FileCategory::Documents,
    ),
    ("application/vnd.ms-powerpoint", FileCategory::Documents),
    (
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        FileCategory::Documents,
    ),
    ("application/rtf", FileCategory::Documents),
    ("application/epub+zip", FileCategory::Documents),
    ("text/plain", FileCategory::Documents),
    ("text/csv", FileCategory::Documents),
    ("text/markdown", FileCategory::Documents),
    ("application/x-msdownload", FileCategory::Programs),
    ("application/x-msi", FileCategory::Programs),
    (
        "application/vnd.android.package-archive",
        FileCategory::Programs,
    ),
    ("application/x-executable", FileCategory::Programs),
    ("application/x-debian-package", FileCategory::Programs),
    (
        "application/x-redhat-package-manager",
        FileCategory::Programs,
    ),
    ("application/java-archive", FileCategory::Programs),
    ("application/zip", FileCategory::Compressed),
    ("application/x-rar-compressed", FileCategory::Compressed),
    ("application/x-7z-compressed", FileCategory::Compressed),
    ("application/x-tar", FileCategory::Compressed),
    ("application/gzip", FileCategory::Compressed),
    ("application/x-bzip2", FileCategory::Compressed),
    ("application/x-xz", FileCategory::Compressed),
    ("application/x-iso9660-image", FileCategory::Compressed),
    ("application/x-apple-diskimage", FileCategory::Compressed),
];

/// Functional file category used to pick a download subfolder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileCategory {
    /// Video and movie files.
    Video,
    /// Music and audio files.
    Audio,
    /// Documents, spreadsheets and e-books.
    Documents,
    /// Installers and executables.
    Programs,
    /// Archives and disk images.
    Compressed,
    /// Everything the tables above do not recognise.
    Other,
}

impl FileCategory {
    /// Every category, in the display order used by the frontend.
    pub const ALL: [FileCategory; 6] = [
        FileCategory::Programs,
        FileCategory::Video,
        FileCategory::Audio,
        FileCategory::Documents,
        FileCategory::Compressed,
        FileCategory::Other,
    ];

    /// Stable identifier shared with the frontend (`programs`, `video`, ...).
    ///
    /// This is also the key used by the per-category folder overrides in the
    /// desktop settings, so it must not change without a settings migration.
    pub fn as_str(self) -> &'static str {
        match self {
            FileCategory::Programs => "programs",
            FileCategory::Video => "video",
            FileCategory::Audio => "audio",
            FileCategory::Documents => "documents",
            FileCategory::Compressed => "compressed",
            FileCategory::Other => "other",
        }
    }

    /// Subfolder created below the download directory for this category.
    pub fn subfolder(self) -> &'static str {
        match self {
            FileCategory::Programs => "Programs",
            FileCategory::Video => "Video",
            FileCategory::Audio => "Music",
            FileCategory::Documents => "Documents",
            FileCategory::Compressed => "Compressed",
            FileCategory::Other => "Other",
        }
    }

    /// Parses a frontend category id; unknown ids fall back to [`Self::Other`].
    pub fn from_id(id: &str) -> Self {
        let normalized = id.trim().to_lowercase();
        Self::ALL
            .iter()
            .copied()
            .find(|category| category.as_str() == normalized)
            .unwrap_or(FileCategory::Other)
    }

    /// Maps a file extension (without the dot) to its category.
    pub fn from_extension(extension: &str) -> Option<Self> {
        let normalized = extension.trim().to_lowercase();
        if normalized.is_empty() {
            return None;
        }
        EXTENSION_TABLE
            .iter()
            .find(|(known, _)| *known == normalized.as_str())
            .map(|(_, category)| *category)
    }

    /// Maps an HTTP `Content-Type` value to its category.
    pub fn from_mime(mime: &str) -> Option<Self> {
        let normalized = mime
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        if normalized.is_empty() {
            return None;
        }
        let matched = MIME_TABLE.iter().find(|(pattern, _)| {
            if pattern.ends_with('/') {
                normalized.starts_with(*pattern)
            } else {
                normalized.as_str() == *pattern
            }
        });
        matched.map(|(_, category)| *category)
    }

    /// Appends this category's subfolder to `base`.
    pub fn folder_in(self, base: &Path) -> PathBuf {
        base.join(self.subfolder())
    }
}

/// Extracts the lowercase extension of a file name, without the dot.
///
/// Query strings and fragments are ignored, and a trailing dot yields no
/// extension, matching `extractExtension` in `classify.js`.
pub fn extract_extension(filename: &str) -> String {
    let head = filename.split(['?', '#']).next().unwrap_or_default();
    match head.trim().rsplit_once('.') {
        Some((_, extension)) => extension.trim().to_lowercase(),
        None => String::new(),
    }
}

/// Classifies a download by file name, falling back to its MIME type.
///
/// Mirrors `classifyFile` in `classify.js`: an unknown or missing extension
/// falls through to the MIME table and finally to [`FileCategory::Other`].
pub fn categorize(filename: &str, mime: Option<&str>) -> FileCategory {
    let extension = extract_extension(filename);
    if let Some(category) = FileCategory::from_extension(&extension) {
        return category;
    }
    mime.and_then(FileCategory::from_mime)
        .unwrap_or(FileCategory::Other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn test_extension_table_maps_every_row() {
        for (extension, expected) in EXTENSION_TABLE {
            let filename = format!("sample.{extension}");
            assert_eq!(categorize(&filename, None), *expected);
            let upper = format!("SAMPLE.{}?token=abc#t=10", extension.to_uppercase());
            assert_eq!(categorize(&upper, None), *expected);
        }
    }

    #[test]
    fn test_extension_table_covers_all_six_categories() {
        for category in FileCategory::ALL {
            assert!(EXTENSION_TABLE
                .iter()
                .any(|(_, mapped)| *mapped == category));
        }
    }

    #[test]
    fn test_known_extensions_map_to_the_expected_category() {
        assert_eq!(categorize("movie.mkv", None), FileCategory::Video);
        assert_eq!(categorize("song.flac", None), FileCategory::Audio);
        assert_eq!(categorize("report.docx", None), FileCategory::Documents);
        assert_eq!(categorize("setup.msi", None), FileCategory::Programs);
        assert_eq!(categorize("backup.7z", None), FileCategory::Compressed);
        assert_eq!(categorize("MOVIE.MP4", None), FileCategory::Video);
        assert_eq!(categorize("movie.mp4?token=1", None), FileCategory::Video);
    }

    #[test]
    fn test_unknown_and_missing_extension_fall_back_to_other() {
        assert_eq!(categorize("backup.qqq", None), FileCategory::Other);
        assert_eq!(categorize("no_extension", None), FileCategory::Other);
        assert_eq!(categorize("trailing_dot.", None), FileCategory::Other);
        assert_eq!(categorize("", None), FileCategory::Other);
        assert_eq!(FileCategory::from_extension("qqq"), None);
        assert_eq!(FileCategory::from_extension("  "), None);
    }

    #[test]
    fn test_mime_fallback_when_extension_is_unknown() {
        let text_plain = Some("text/plain; charset=utf-8");
        assert_eq!(
            categorize("clip.qqq", Some("video/mp4")),
            FileCategory::Video
        );
        assert_eq!(categorize("song", Some("audio/mpeg")), FileCategory::Audio);
        assert_eq!(categorize("x.bin", text_plain), FileCategory::Documents);
        assert_eq!(
            categorize("x.qqq", Some("application/zip")),
            FileCategory::Compressed
        );
        assert_eq!(
            categorize("x.qqq", Some("application/x-msi")),
            FileCategory::Programs
        );
        // A known extension always wins over the MIME type.
        assert_eq!(
            categorize("photo.mp3", Some("video/mp4")),
            FileCategory::Audio
        );
        // Unrecognised MIME types end in `other`.
        assert_eq!(
            categorize("blob", Some("application/octet-stream")),
            FileCategory::Other
        );
        assert_eq!(categorize("blob", Some("   ")), FileCategory::Other);
    }

    #[test]
    fn test_category_subfolder_names() {
        assert_eq!(FileCategory::Programs.subfolder(), "Programs");
        assert_eq!(FileCategory::Video.subfolder(), "Video");
        assert_eq!(FileCategory::Audio.subfolder(), "Music");
        assert_eq!(FileCategory::Documents.subfolder(), "Documents");
        assert_eq!(FileCategory::Compressed.subfolder(), "Compressed");
        assert_eq!(FileCategory::Other.subfolder(), "Other");
    }

    #[test]
    fn test_folder_in_appends_the_category_subfolder() {
        let base = Path::new("C:\\Users\\User\\Downloads");
        for category in FileCategory::ALL {
            let folder = category.folder_in(base);
            assert_eq!(folder.file_name(), Some(OsStr::new(category.subfolder())));
            assert_eq!(folder.parent(), Some(base));
        }
        let other_folder = categorize("mystery.qqq", None).folder_in(base);
        assert_eq!(other_folder, base.join("Other"));
    }

    #[test]
    fn test_category_ids_roundtrip_and_unknown_id_falls_back() {
        for category in FileCategory::ALL {
            assert_eq!(FileCategory::from_id(category.as_str()), category);
        }
        assert_eq!(FileCategory::from_id("  VIDEO "), FileCategory::Video);
        assert_eq!(FileCategory::from_id("all"), FileCategory::Other);
        assert_eq!(FileCategory::from_id("nonsense"), FileCategory::Other);
    }
}

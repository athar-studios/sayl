/**
 * Pure classification module for SAYL download manager.
 * Classifies files into 6 functional categories plus an 'other' fallback:
 * video, audio, documents, programs, compressed, other.
 *
 * Primary classification uses file extension; falls back to HTTP Content-Type (MIME)
 * when the extension is absent or unrecognised.
 */

export const CATEGORIES = [
  { id: "all", labelAr: "الكل", labelEn: "All" },
  { id: "video", labelAr: "فيديو", labelEn: "Video" },
  { id: "audio", labelAr: "صوت", labelEn: "Audio" },
  { id: "documents", labelAr: "مستندات", labelEn: "Documents" },
  { id: "programs", labelAr: "برامج", labelEn: "Programs" },
  { id: "compressed", labelAr: "ملفات مضغوطة", labelEn: "Compressed" },
  { id: "other", labelAr: "أخرى", labelEn: "Other" },
];

export const CATEGORY_IDS = [
  "all",
  "video",
  "audio",
  "documents",
  "programs",
  "compressed",
  "other",
];

const EXT_TO_CATEGORY = {
  // Video
  mp4: "video",
  mkv: "video",
  avi: "video",
  mov: "video",
  webm: "video",
  flv: "video",
  wmv: "video",
  m4v: "video",
  "3gp": "video",
  ts: "video",
  mpg: "video",
  mpeg: "video",
  ogv: "video",

  // Audio
  mp3: "audio",
  wav: "audio",
  flac: "audio",
  aac: "audio",
  ogg: "audio",
  m4a: "audio",
  wma: "audio",
  opus: "audio",
  alac: "audio",
  aiff: "audio",
  mid: "audio",
  midi: "audio",

  // Documents
  pdf: "documents",
  doc: "documents",
  docx: "documents",
  xls: "documents",
  xlsx: "documents",
  ppt: "documents",
  pptx: "documents",
  txt: "documents",
  md: "documents",
  csv: "documents",
  rtf: "documents",
  odt: "documents",
  ods: "documents",
  odp: "documents",
  epub: "documents",

  // Programs
  exe: "programs",
  msi: "programs",
  apk: "programs",
  appimage: "programs",
  deb: "programs",
  rpm: "programs",
  pkg: "programs",
  jar: "programs",
  bat: "programs",
  cmd: "programs",
  com: "programs",

  // Compressed / Archives
  zip: "compressed",
  rar: "compressed",
  "7z": "compressed",
  tar: "compressed",
  gz: "compressed",
  bz2: "compressed",
  xz: "compressed",
  tgz: "compressed",
  iso: "compressed",
  dmg: "compressed",
  cab: "compressed",
  z: "compressed",
  lz: "compressed",
  lzma: "compressed",
};

const MIME_PATTERNS = [
  { prefix: "video/", category: "video" },
  { prefix: "audio/", category: "audio" },
  { exact: "application/pdf", category: "documents" },
  { exact: "application/msword", category: "documents" },
  { exact: "application/vnd.openxmlformats-officedocument.wordprocessingml.document", category: "documents" },
  { exact: "application/vnd.ms-excel", category: "documents" },
  { exact: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", category: "documents" },
  { exact: "application/vnd.ms-powerpoint", category: "documents" },
  { exact: "application/vnd.openxmlformats-officedocument.presentationml.presentation", category: "documents" },
  { exact: "application/rtf", category: "documents" },
  { exact: "application/epub+zip", category: "documents" },
  { exact: "text/plain", category: "documents" },
  { exact: "text/csv", category: "documents" },
  { exact: "text/markdown", category: "documents" },
  { exact: "application/x-msdownload", category: "programs" },
  { exact: "application/x-msi", category: "programs" },
  { exact: "application/vnd.android.package-archive", category: "programs" },
  { exact: "application/x-executable", category: "programs" },
  { exact: "application/x-debian-package", category: "programs" },
  { exact: "application/x-redhat-package-manager", category: "programs" },
  { exact: "application/java-archive", category: "programs" },
  { exact: "application/zip", category: "compressed" },
  { exact: "application/x-rar-compressed", category: "compressed" },
  { exact: "application/x-7z-compressed", category: "compressed" },
  { exact: "application/x-tar", category: "compressed" },
  { exact: "application/gzip", category: "compressed" },
  { exact: "application/x-bzip2", category: "compressed" },
  { exact: "application/x-xz", category: "compressed" },
  { exact: "application/x-iso9660-image", category: "compressed" },
  { exact: "application/x-apple-diskimage", category: "compressed" },
];

/**
 * Extracts clean extension from filename.
 * Returns lowercase extension without dot, or empty string.
 */
function extractExtension(filename) {
  if (!filename || typeof filename !== "string") return "";
  const clean = filename.trim().split(/[?#]/)[0];
  const lastDot = clean.lastIndexOf(".");
  if (lastDot === -1 || lastDot === clean.length - 1) return "";
  return clean.slice(lastDot + 1).trim().toLowerCase();
}

/**
 * Classifies MIME type into category.
 */
function classifyMime(mime) {
  if (!mime || typeof mime !== "string") return null;
  const clean = mime.trim().toLowerCase().split(";")[0].trim();
  if (!clean) return null;

  for (const pattern of MIME_PATTERNS) {
    if (pattern.prefix && clean.startsWith(pattern.prefix)) {
      return pattern.category;
    }
    if (pattern.exact && clean === pattern.exact) {
      return pattern.category;
    }
  }

  return null;
}

/**
 * Classifies a file into one of: 'video', 'audio', 'documents', 'programs', 'compressed', 'other'.
 * Priority:
 * 1. Extension match
 * 2. MIME fallback (if extension absent or not recognised)
 * 3. 'other'
 *
 * @param {string|null|undefined} filename
 * @param {string|null|undefined} mime
 * @returns {string} Category identifier
 */
export function classifyFile(filename, mime) {
  const ext = extractExtension(filename);
  if (ext && EXT_TO_CATEGORY[ext]) {
    return EXT_TO_CATEGORY[ext];
  }

  const mimeCategory = classifyMime(mime);
  if (mimeCategory) {
    return mimeCategory;
  }

  return "other";
}

/**
 * Returns Arabic label for a category.
 */
export function getCategoryLabel(category, lang = "ar") {
  const found = CATEGORIES.find((c) => c.id === category);
  if (!found) return lang === "ar" ? "أخرى" : "Other";
  return lang === "ar" ? found.labelAr : found.labelEn;
}

/**
 * Computes per-category task counts.
 * Returns an object with counts for 'all', 'video', 'audio', 'documents', 'programs', 'compressed', 'other'.
 *
 * @param {Array<{ filename?: string, mime?: string }>} tasks
 * @returns {Record<string, number>}
 */
export function getCategoryCounts(tasks) {
  const counts = {
    all: 0,
    video: 0,
    audio: 0,
    documents: 0,
    programs: 0,
    compressed: 0,
    other: 0,
  };

  if (!Array.isArray(tasks)) return counts;

  counts.all = tasks.length;
  for (const task of tasks) {
    const cat = classifyFile(task?.filename, task?.mime);
    if (Object.prototype.hasOwnProperty.call(counts, cat)) {
      counts[cat] += 1;
    } else {
      counts.other += 1;
    }
  }

  return counts;
}

// Alias for flexibility
export const countByCategory = getCategoryCounts;

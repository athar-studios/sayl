// SAYL Download Interception Rules - Pure Decision Logic

export const DEFAULT_EXTENSIONS = [
  "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "iso", "img", "dmg",
  "exe", "msi", "msix", "apk", "pkg", "bin", "dat", "mp4", "mkv", "avi",
  "mov", "wmv", "flv", "webm", "mp3", "flac", "wav", "pdf"
];

export const DEFAULT_MIN_SIZE_MB = 1;

/** Browser temp/in-progress download extensions to ignore when choosing a name. */
export const TEMP_DOWNLOAD_EXTENSIONS = new Set([
  "crdownload",
  "tmp",
  "partial",
  ""
]);

/**
 * @param {string} filename
 * @returns {boolean}
 */
export function isTempDownloadName(filename) {
  const ext = extractExtension(filename);
  return TEMP_DOWNLOAD_EXTENSIONS.has(ext);
}

/**
 * Normalizes an array or comma-separated string into a Set of lowercase extensions without leading dots.
 * @param {string[]|string|Set<string>} input
 * @returns {Set<string>}
 */
export function parseExtensionList(input) {
  if (!input) return new Set(DEFAULT_EXTENSIONS);
  const items = Array.isArray(input)
    ? input
    : input instanceof Set
      ? Array.from(input)
      : String(input).split(",");

  const set = new Set();
  for (const raw of items) {
    const trimmed = String(raw).trim().toLowerCase().replace(/^\.+/, "");
    if (trimmed.length > 0) {
      set.add(trimmed);
    }
  }
  return set.size > 0 ? set : new Set(DEFAULT_EXTENSIONS);
}

/**
 * Extracts the file basename from a path or URL string.
 * @param {string} pathOrUrl
 * @returns {string}
 */
export function extractBasename(pathOrUrl) {
  if (!pathOrUrl || typeof pathOrUrl !== "string") return "";
  let clean = pathOrUrl.trim();
  // Strip fragment and query if URL-like
  if (clean.includes("#")) {
    clean = clean.split("#")[0];
  }
  if (clean.includes("?")) {
    clean = clean.split("?")[0];
  }
  // Normalize Windows backslashes
  clean = clean.replace(/\\/g, "/");
  // Remove trailing slashes
  clean = clean.replace(/\/+$/, "");
  const lastSlash = clean.lastIndexOf("/");
  const base = lastSlash >= 0 ? clean.slice(lastSlash + 1) : clean;
  try {
    return decodeURIComponent(base);
  } catch {
    return base;
  }
}

/**
 * Extracts the file extension (without dot, lowercase) from a filename.
 * @param {string} filename
 * @returns {string}
 */
export function extractExtension(filename) {
  const base = extractBasename(filename);
  if (!base) return "";
  const lastDot = base.lastIndexOf(".");
  if (lastDot <= 0 || lastDot === base.length - 1) {
    return "";
  }
  return base.slice(lastDot + 1).toLowerCase();
}

/**
 * Checks whether the filename extension is in the allowed list.
 * @param {string} filename
 * @param {string[]|string|Set<string>} allowed
 * @returns {boolean}
 */
export function matchesExtension(filename, allowed = DEFAULT_EXTENSIONS) {
  const ext = extractExtension(filename);
  if (!ext) return false;
  const allowSet = parseExtensionList(allowed);
  return allowSet.has(ext);
}

/**
 * Checks whether the download size passes the minimum size threshold.
 * Unknown size (-1 or 0) passes the size check.
 * @param {number|null|undefined} bytes
 * @param {number} minSizeMb
 * @returns {boolean}
 */
export function passesSize(bytes, minSizeMb = DEFAULT_MIN_SIZE_MB) {
  if (bytes === null || bytes === undefined || typeof bytes !== "number") {
    return true;
  }
  // Unknown size (-1 or 0) passes
  if (bytes <= 0) {
    return true;
  }
  const minBytes = Math.max(0, minSizeMb) * 1024 * 1024;
  return bytes >= minBytes;
}

/**
 * Checks if a URL was recorded as an Alt-click bypass within TTL.
 * @param {string} url
 * @param {Record<string, number>|null|undefined} bypassRecords
 * @param {number} nowMs
 * @param {number} ttlMs
 * @returns {boolean}
 */
export function isAltBypassed(url, bypassRecords, nowMs = Date.now(), ttlMs = 10000) {
  if (!url || !bypassRecords || typeof bypassRecords !== "object") {
    return false;
  }

  // Exact match
  if (typeof bypassRecords[url] === "number") {
    const age = nowMs - bypassRecords[url];
    if (age >= 0 && age <= ttlMs) {
      return true;
    }
  }

  // Clean URL match (strip hash and query)
  const cleanUrl = url.split("#")[0].split("?")[0];
  if (cleanUrl !== url && typeof bypassRecords[cleanUrl] === "number") {
    const age = nowMs - bypassRecords[cleanUrl];
    if (age >= 0 && age <= ttlMs) {
      return true;
    }
  }

  // Check any stored key that matches the URL
  for (const [storedUrl, timestamp] of Object.entries(bypassRecords)) {
    if (typeof timestamp !== "number") continue;
    const age = nowMs - timestamp;
    if (age < 0 || age > ttlMs) continue;
    if (storedUrl === url || storedUrl.split("#")[0].split("?")[0] === cleanUrl) {
      return true;
    }
  }

  return false;
}

/**
 * Cleans a URL for logging: origin + basename only, no query parameters or cookies.
 * @param {string} rawUrl
 * @returns {string}
 */
export function sanitizeUrlForLog(rawUrl) {
  if (!rawUrl || typeof rawUrl !== "string") return "";
  try {
    const parsed = new URL(rawUrl);
    const parts = parsed.pathname.split("/").filter(Boolean);
    const basename = parts.length > 0 ? parts[parts.length - 1] : "";
    return `${parsed.origin}${basename ? "/" + basename : ""}`;
  } catch {
    const clean = rawUrl.split("?")[0].split("#")[0];
    const lastSlash = clean.lastIndexOf("/");
    return lastSlash >= 0 ? clean.slice(0, lastSlash + 1) + extractBasename(clean) : clean;
  }
}

/**
 * Evaluates whether a download item should be intercepted by SAYL.
 * @param {object} params
 * @param {object} params.item chrome.downloads.DownloadItem
 * @param {object} [params.settings] { enabled, extensions, minSizeMb }
 * @param {Record<string, number>} [params.altBypasses]
 * @param {string} [params.extensionId]
 * @param {string} [params.suggestedFilename] from onDeterminingFilename
 * @param {number} [params.nowMs]
 * @returns {{ intercept: boolean, reason: string, filename: string, size: number, targetUrl: string }}
 */
export function shouldInterceptDownload({
  item,
  settings,
  altBypasses,
  extensionId,
  suggestedFilename,
  nowMs = Date.now()
}) {
  if (!item) {
    return { intercept: false, reason: "invalid_item", filename: "", size: 0, targetUrl: "" };
  }

  const enabled = settings?.enabled ?? true;
  if (!enabled) {
    return { intercept: false, reason: "disabled", filename: "", size: 0, targetUrl: "" };
  }

  if (extensionId && item.byExtensionId === extensionId) {
    return { intercept: false, reason: "own_download", filename: "", size: 0, targetUrl: "" };
  }

  const targetUrl = item.finalUrl || item.url || "";
  if (!targetUrl || !/^https?:\/\//i.test(targetUrl)) {
    return { intercept: false, reason: "non_http_url", filename: "", size: 0, targetUrl };
  }

  if (isAltBypassed(targetUrl, altBypasses, nowMs, 10000) || (item.url && isAltBypassed(item.url, altBypasses, nowMs, 10000))) {
    return { intercept: false, reason: "alt_bypass", filename: "", size: 0, targetUrl };
  }

  const allowedExtensions = settings?.extensions ?? DEFAULT_EXTENSIONS;
  const urlBasename = extractBasename(targetUrl);
  let filename = urlBasename || "download";

  if (matchesExtension(urlBasename, allowedExtensions)) {
    filename = urlBasename;
  } else {
    const suggestedBase = suggestedFilename
      ? extractBasename(suggestedFilename)
      : "";
    const localBase = item.filename ? extractBasename(item.filename) : "";

    if (suggestedBase && !isTempDownloadName(suggestedBase)) {
      filename = suggestedBase;
    } else if (localBase && !isTempDownloadName(localBase)) {
      filename = localBase;
    } else {
      filename = urlBasename || localBase || "download";
    }
  }

  if (!matchesExtension(filename, allowedExtensions)) {
    return { intercept: false, reason: "non_listed_extension", filename, size: 0, targetUrl };
  }

  const size = (typeof item.fileSize === "number" && item.fileSize > 0)
    ? item.fileSize
    : (typeof item.totalBytes === "number" && item.totalBytes > 0)
      ? item.totalBytes
      : 0;

  const minSizeMb = settings?.minSizeMb ?? DEFAULT_MIN_SIZE_MB;
  if (!passesSize(size, minSizeMb)) {
    return { intercept: false, reason: "too_small", filename, size, targetUrl };
  }

  return { intercept: true, reason: "allowed", filename, size, targetUrl };
}

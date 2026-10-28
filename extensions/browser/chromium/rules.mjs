// SAYL Download Interception Rules - Pure Decision Logic

export const DEFAULT_EXTENSIONS = [
  "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "iso", "img", "dmg",
  "exe", "msi", "msix", "apk", "pkg", "bin", "dat", "mp4", "mkv", "avi",
  "mov", "wmv", "flv", "webm", "mp3", "flac", "wav", "pdf",
  "docx", "xlsx", "pptx", "epub", "tgz", "zst", "deb", "rpm",
  "appimage", "jar", "csv", "doc", "xls", "ppt", "odt", "ods", "odp"
];

export const DEFAULT_MIN_SIZE_MB = 1;

/** Browser temp/in-progress download extensions to ignore when choosing a name. */
export const TEMP_DOWNLOAD_EXTENSIONS = new Set([
  "crdownload",
  "tmp",
  "partial"
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
/** Host suffixes used for GitHub release/raw redirect targets (signed, often extensionless paths). */
export const GITHUB_REDIRECT_HOST_SUFFIXES = [
  ".githubusercontent.com",
  "codeload.github.com",
];

/**
 * @param {string} rawUrl
 * @returns {boolean}
 */
export function isGithubRedirectUrl(rawUrl) {
  if (!rawUrl || typeof rawUrl !== "string") return false;
  try {
    const host = new URL(rawUrl).hostname.toLowerCase();
    return GITHUB_REDIRECT_HOST_SUFFIXES.some(
      (suffix) => host === suffix.replace(/^\./, "") || host.endsWith(suffix)
    );
  } catch {
    return false;
  }
}

/**
 * URLs whose last path segment is frequently extensionless but that always serve a file:
 * GitHub release downloads, source archives, and the signed CDN hosts they redirect to.
 * @param {string} rawUrl
 * @returns {boolean}
 */
export function isGithubReleaseUrl(rawUrl) {
  if (!rawUrl || typeof rawUrl !== "string") return false;
  // Signed release/archive CDNs (objects.githubusercontent.com,
  // release-assets.githubusercontent.com, codeload.github.com).
  if (isGithubRedirectUrl(rawUrl)) return true;
  try {
    const parsed = new URL(rawUrl);
    const host = parsed.hostname.toLowerCase();
    if (host !== "github.com" && host !== "www.github.com") return false;
    const path = parsed.pathname;
    // github.com/<owner>/<repo>/releases/download/<tag>/<asset>
    // github.com/<owner>/<repo>/archive/<ref>.zip|tar.gz
    return /^\/[^/]+\/[^/]+\/(releases\/download|archive)\//i.test(path);
  } catch {
    return false;
  }
}

/** Google hosts that return a file only after a confirm step. */
const GOOGLE_DRIVE_HOSTS = new Set([
  "drive.usercontent.google.com",
  "drive.google.com",
  "docs.google.com"
]);

/**
 * Google Drive / Docs download URLs, including the `uc?export=download` confirm endpoint.
 * @param {string} rawUrl
 * @returns {boolean}
 */
export function isGoogleDriveUrl(rawUrl) {
  if (!rawUrl || typeof rawUrl !== "string") return false;
  try {
    const parsed = new URL(rawUrl);
    const host = parsed.hostname.toLowerCase();
    if (!GOOGLE_DRIVE_HOSTS.has(host)) return false;
    if (host === "drive.usercontent.google.com") return true;
    // drive.google.com/... and docs.google.com/uc?export=download are confirm endpoints.
    const exportParam = parsed.searchParams.get("export");
    if (exportParam && exportParam.toLowerCase() === "download") return true;
    if (host === "docs.google.com") {
      return parsed.pathname === "/uc" || parsed.pathname.startsWith("/uc/");
    }
    return true;
  } catch {
    return false;
  }
}

/** MIME types that always mean "this is a file, not a page". */
export const BINARY_MIME_TYPES = new Set([
  "application/octet-stream",
  "application/zip",
  "application/x-zip-compressed",
  "application/x-rar-compressed",
  "application/vnd.rar",
  "application/x-7z-compressed",
  "application/x-tar",
  "application/gzip",
  "application/x-gzip",
  "application/x-bzip2",
  "application/x-xz",
  "application/zstd",
  "application/x-msdownload",
  "application/x-msi",
  "application/vnd.microsoft.portable-executable",
  "application/vnd.android.package-archive",
  "application/x-apple-diskimage",
  "application/x-debian-package",
  "application/x-rpm",
  "application/java-archive",
  "application/x-iso9660-image",
  "application/appimage",
  "application/pdf",
  "application/epub+zip",
  "application/msword",
  "application/vnd.ms-excel",
  "application/vnd.ms-powerpoint",
  "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
  "application/vnd.openxmlformats-officedocument.presentationml.presentation"
]);

/** Prefixes (before the `/`) that always mean binary media payloads. */
const BINARY_MIME_PREFIXES = ["video/", "audio/"];

/**
 * Normalizes a Content-Type value to `type/subtype` in lowercase.
 * @param {string} mime
 * @returns {string}
 */
export function normalizeMime(mime) {
  if (!mime || typeof mime !== "string") return "";
  return mime.split(";")[0].trim().toLowerCase();
}

/**
 * @param {string} mime
 * @returns {boolean}
 */
export function isBinaryMime(mime) {
  const type = normalizeMime(mime);
  if (!type) return false;
  if (BINARY_MIME_TYPES.has(type)) return true;
  return BINARY_MIME_PREFIXES.some((prefix) => type.startsWith(prefix));
}

/**
 * True when the server answered with an HTML page instead of a file
 * (login wall, Drive virus-scan confirm page, expired link).
 * @param {string} mime
 * @returns {boolean}
 */
export function isHtmlMime(mime) {
  const type = normalizeMime(mime);
  return type === "text/html" || type === "application/xhtml+xml";
}

/** Arabic message shown when a link serves an HTML page instead of a file. */
export const HTML_RESPONSE_MESSAGE_AR =
  "الرابط أعاد صفحة HTML بدلًا من ملف. غالبًا انتهت صلاحية الرابط أو يتطلب تسجيل الدخول. لم يُنشأ أي تنزيل.";

/** Host error codes that mean "the server returned HTML, not a file". */
export const HTML_FAILURE_ERROR_CODES = new Set([
  "html_response",
  "not_a_file",
  "invalid_content_type",
  "content_type_html"
]);

/**
 * Determines whether a download item should bypass the extension allowlist
 * and minimum-size check (when size is unknown). Covers:
 * - GitHub releases, source archives, and their signed CDN redirects
 * - Google Drive / Docs confirm-download endpoints
 * - Responses with a binary MIME (application/octet-stream, video/*, audio/*, etc.)
 * @param {object} item
 * @returns {boolean}
 */
export function isForceCaptureItem(item) {
  if (!item) return false;
  const urls = [item.finalUrl, item.url].filter(Boolean);
  if (urls.some(isGithubReleaseUrl)) return true;
  if (urls.some(isGoogleDriveUrl)) return true;
  return isBinaryMime(item.mime);
}

/** Schemes that must never be handed to the download engine. */
const BLOCKED_URL_SCHEMES = ["blob:", "data:", "filesystem:", "chrome-extension:"];

/**
 * @param {string} rawUrl
 * @returns {boolean}
 */
export function isBlockedUrlScheme(rawUrl) {
  if (!rawUrl || typeof rawUrl !== "string") return false;
  const lower = rawUrl.trim().toLowerCase();
  return BLOCKED_URL_SCHEMES.some((scheme) => lower.startsWith(scheme));
}

/**
 * Picks the best filename for intercept rules when finalUrl is a CDN redirect without an extension.
 * @param {object} item
 * @param {string|undefined} suggestedFilename
 * @param {string[]|string|Set<string>} allowedExtensions
 * @returns {string}
 */
export function resolveInterceptFilename(item, suggestedFilename, allowedExtensions) {
  const targetUrl = item?.finalUrl || item?.url || "";
  const sourceUrl = item?.url || "";

  /** @type {string[]} */
  const candidates = [];

  const pushName = (raw) => {
    if (!raw || typeof raw !== "string") return;
    const base = extractBasename(raw);
    if (!base || isTempDownloadName(base)) return;
    if (!candidates.includes(base)) {
      candidates.push(base);
    }
  };

  // Priority: the name the browser determined (onDeterminingFilename / item.filename),
  // then the last path segment of the final URL, then of the original URL, then "download".
  if (item?.filename) {
    pushName(extractBasename(item.filename) || item.filename);
  }
  if (suggestedFilename) {
    pushName(suggestedFilename);
  }
  pushName(targetUrl);
  if (sourceUrl && sourceUrl !== targetUrl) {
    pushName(sourceUrl);
  }

  for (const base of candidates) {
    if (matchesExtension(base, allowedExtensions)) {
      return base;
    }
  }

  if (candidates.length > 0) {
    return candidates[0];
  }

  // Last URL path segment, even when it carries no usable extension.
  const urlBase = extractBasename(targetUrl) || extractBasename(sourceUrl);
  return urlBase && !isTempDownloadName(urlBase) ? urlBase : "download";
}

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
 * Dedupe cache keyed by `downloadId::cleanUrl`.
 * Module level so tests can inspect and reset it.
 */
export const urlDedupeCache = new Map();
export const DEDUPE_WINDOW_MS = 3_000;

/**
 * Builds the dedupe key for one browser download event.
 * The download id keeps two independent downloads of the same URL from
 * cancelling each other; the URL is stripped of fragment/query noise.
 * @param {string} url
 * @param {string|number} [downloadId]
 * @returns {string}
 */
export function buildDedupeKey(url, downloadId) {
  const cleanUrl = String(url || "").split(/[?#]/)[0];
  if (downloadId === undefined || downloadId === null || downloadId === "") {
    return cleanUrl;
  }
  return `${downloadId}::${cleanUrl}`;
}

/**
 * Pure read: reports whether this download was already intercepted recently.
 * Never records anything — recording happens only after a real intercept.
 * @param {string} url
 * @param {number} nowMs
 * @param {string|number} [downloadId]
 * @returns {boolean}
 */
export function isUrlDeduped(url, nowMs = Date.now(), downloadId) {
  if (!url) return false;
  const key = buildDedupeKey(url, downloadId);
  const lastSeen = urlDedupeCache.get(key);
  if (typeof lastSeen !== "number") return false;
  const age = nowMs - lastSeen;
  return age >= 0 && age < DEDUPE_WINDOW_MS;
}

/**
 * Records a successful intercept. Call this only after the capture was accepted
 * (or at least after the browser download was cancelled), never while deciding.
 * @param {string} url
 * @param {number} nowMs
 * @param {string|number} [downloadId]
 */
export function recordUrlDedupe(url, nowMs = Date.now(), downloadId) {
  if (!url) return;
  urlDedupeCache.set(buildDedupeKey(url, downloadId), nowMs);
  // Prune stale entries periodically
  if (urlDedupeCache.size > 1000) {
    for (const [key, t] of urlDedupeCache.entries()) {
      if (typeof t !== "number" || nowMs - t > DEDUPE_WINDOW_MS) {
        urlDedupeCache.delete(key);
      }
    }
  }
}

export function resetUrlDedupeCache() {
  urlDedupeCache.clear();
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
 * @param {Set<string>} [params.givenBackUrls]
 * @param {boolean} [params.checkDedupe]
 * @returns {{ intercept: boolean, reason: string, filename: string, size: number, targetUrl: string, forced?: boolean, message?: string }}
 */
export function shouldInterceptDownload({
  item,
  settings,
  altBypasses,
  extensionId,
  suggestedFilename,
  nowMs = Date.now(),
  givenBackUrls,
  checkDedupe = false,
}) {
  if (!item) {
    return { intercept: false, reason: "invalid_item", filename: "", size: 0, targetUrl: "" };
  }

  const enabled = settings?.enabled ?? true;
  if (!enabled) {
    return { intercept: false, reason: "disabled", filename: "", size: 0, targetUrl: "" };
  }

  // Guard 1: own download triggered by this extension
  if (extensionId && item.byExtensionId === extensionId) {
    return { intercept: false, reason: "own_download", filename: "", size: 0, targetUrl: "" };
  }

  // Guard 2: other extension / restored download
  if (item.byExtensionId && (!extensionId || item.byExtensionId !== extensionId)) {
    return { intercept: false, reason: "restored_download", filename: "", size: 0, targetUrl: "" };
  }

  // Guard 3: old download (>30s)
  if (item.startTime) {
    const startMs = new Date(item.startTime).getTime();
    if (!isNaN(startMs) && (nowMs - startMs > 30_000)) {
      return { intercept: false, reason: "old_download", filename: "", size: 0, targetUrl: "" };
    }
  }

  const targetUrl = item.finalUrl || item.url || "";
  // Never hand blob:, data:, filesystem: or any other non-http(s) URL to the engine.
  if (!targetUrl || !/^https?:\/\//i.test(targetUrl)) {
    return { intercept: false, reason: "non_http_url", filename: "", size: 0, targetUrl };
  }
  if (isBlockedUrlScheme(targetUrl) || isBlockedUrlScheme(item.url)) {
    return { intercept: false, reason: "non_http_url", filename: "", size: 0, targetUrl };
  }

  // Guard 4: given back to browser once
  if (givenBackUrls && (givenBackUrls.has(targetUrl) || (item.url && givenBackUrls.has(item.url)))) {
    return { intercept: false, reason: "given_back_to_browser", filename: "", size: 0, targetUrl };
  }

  // Guard 5: same-download dedupe (pure read — recording happens after the intercept).
  if (checkDedupe && isUrlDeduped(targetUrl, nowMs, item.id)) {
    return { intercept: false, reason: "url_deduped", filename: "", size: 0, targetUrl };
  }

  if (isAltBypassed(targetUrl, altBypasses, nowMs, 10000) || (item.url && isAltBypassed(item.url, altBypasses, nowMs, 10000))) {
    return { intercept: false, reason: "alt_bypass", filename: "", size: 0, targetUrl };
  }

  // Guard 6: the server answered with an HTML page, not a file. Never intercept,
  // and never retry/give back — the caller reports it to the user in Arabic.
  if (isHtmlMime(item.mime)) {
    return {
      intercept: false,
      reason: "html_response",
      filename: "",
      size: 0,
      targetUrl,
      message: HTML_RESPONSE_MESSAGE_AR
    };
  }

  const allowedExtensions = settings?.extensions ?? DEFAULT_EXTENSIONS;
  const filename = resolveInterceptFilename(item, suggestedFilename, allowedExtensions);

  // GitHub releases, Drive confirm endpoints and binary MIME do not need a listed
  // extension: those responses are files even when the URL path has no suffix.
  const forced = isForceCaptureItem(item);

  if (!forced && !matchesExtension(filename, allowedExtensions)) {
    return { intercept: false, reason: "non_listed_extension", filename, size: 0, targetUrl };
  }

  const size = (typeof item.fileSize === "number" && item.fileSize > 0)
    ? item.fileSize
    : (typeof item.totalBytes === "number" && item.totalBytes > 0)
      ? item.totalBytes
      : 0;

  const minSizeMb = settings?.minSizeMb ?? DEFAULT_MIN_SIZE_MB;
  // For forced captures an unknown size must never block the intercept.
  const sizeUnknown = !(typeof size === "number") || size <= 0;
  if (!(forced && sizeUnknown) && !passesSize(size, minSizeMb)) {
    return { intercept: false, reason: "too_small", filename, size, targetUrl };
  }

  return { intercept: true, reason: "allowed", filename, size, targetUrl, forced };
}

/**
 * Executes the complete download capture pipeline:
 * 1. Checks guards and rules.
 * 2. On intercept: immediately cancels and erases the browser download (never keeps it).
 * 3. Sends capture payload to SAYL native host.
 * 4. Only if host send fails: re-downloads once in the browser via downloadsApi.download.
 */
export async function processDownloadCapture({
  item,
  settings,
  altBypasses,
  extensionId,
  suggestedFilename,
  nowMs = Date.now(),
  givenBackUrls = new Set(),
  downloadsApi,
  sendCaptureFn,
  cookies,
  userAgent,
  referrer
}) {
  const decision = shouldInterceptDownload({
    item,
    settings,
    altBypasses,
    extensionId,
    suggestedFilename,
    nowMs,
    givenBackUrls,
    checkDedupe: true
  });

  if (!decision.intercept) {
    if (decision.reason === "given_back_to_browser") {
      givenBackUrls.delete(decision.targetUrl);
      if (item.url) givenBackUrls.delete(item.url);
    }
    return {
      intercept: false,
      reason: decision.reason,
      cancelled: false,
      // HTML responses are terminal: the caller shows this Arabic message and
      // never hands the download back to the browser.
      message: decision.reason === "html_response" ? HTML_RESPONSE_MESSAGE_AR : undefined
    };
  }

  // 1. Immediately cancel and erase browser download so it never runs in both.
  // chrome.downloads.cancel(id) is the very first await, then erase.
  // If the item already completed, chrome.downloads.removeFile(id) before erase.
  let cancelled = false;
  let removedFile = false;
  let erased = false;
  if (downloadsApi?.cancel) {
    await downloadsApi.cancel(item.id);
    cancelled = true;
  }
  if (item?.state === "complete" && downloadsApi?.removeFile) {
    await downloadsApi.removeFile(item.id);
    removedFile = true;
  }
  if (downloadsApi?.erase) {
    await downloadsApi.erase({ id: item.id });
    erased = true;
  }

  // Record dedupe only now that the download was really intercepted.
  recordUrlDedupe(decision.targetUrl, nowMs, item.id);
  if (item.url && item.url !== decision.targetUrl) {
    recordUrlDedupe(item.url, nowMs, item.id);
  }

  // 2. Send to native host
  let hostResult = null;
  if (sendCaptureFn) {
    const payload = {
      url: item.url || decision.targetUrl,
      final_url: decision.targetUrl,
      filename: decision.filename,
      size: decision.size
    };
    if (referrer !== undefined) payload.referrer = referrer ?? item.referrer ?? "";
    if (cookies !== undefined) payload.cookies = Array.isArray(cookies) ? cookies : [];
    if (userAgent !== undefined) payload.user_agent = userAgent;
    if (item.mime) payload.mime = item.mime;
    hostResult = await sendCaptureFn(payload);
  }

  // 3. Give back to browser ONLY when the host is unreachable (connect error).
  // Never on timeout, never after any other reply, and never for an HTML response:
  // re-downloading an HTML page would just loop.
  let redownloaded = false;
  const isConnectError =
    hostResult &&
    !hostResult.ok &&
    (hostResult.error?.code === "HOST_UNREACHABLE" ||
     hostResult.error?.code === "CONNECT_ERROR");

  if (isConnectError) {
    givenBackUrls.add(decision.targetUrl);
    if (item.url) givenBackUrls.add(item.url);
    if (downloadsApi?.download) {
      await downloadsApi.download({
        url: decision.targetUrl,
        filename: decision.filename || undefined
      });
      redownloaded = true;
    }
  }

  const htmlFailure = Boolean(
    hostResult && !hostResult.ok && HTML_FAILURE_ERROR_CODES.has(String(hostResult.error?.code || ""))
  );

  return {
    intercept: true,
    decision,
    cancelled,
    removedFile,
    erased,
    hostResult,
    redownloaded,
    // Terminal HTML failure: show this Arabic message, never give the download back.
    message: htmlFailure ? HTML_RESPONSE_MESSAGE_AR : undefined
  };
}

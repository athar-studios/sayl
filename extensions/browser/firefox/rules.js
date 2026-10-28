// SAYL Download Interception Rules - shared decision logic for the Firefox
// background script (classic script, intentionally no ES module syntax so it
// loads on every supported Gecko version). Exposes a single global, `SaylRules`,
// mirroring the pure logic used by the Chromium build.

(function (global) {
  const DEFAULT_EXTENSIONS = [
    "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "iso", "img", "dmg",
    "exe", "msi", "msix", "apk", "pkg", "bin", "dat", "mp4", "mkv", "avi",
    "mov", "wmv", "flv", "webm", "mp3", "flac", "wav", "pdf",
    "docx", "xlsx", "pptx", "epub", "tgz", "zst", "deb", "rpm",
    "appimage", "jar", "csv", "doc", "xls", "ppt", "odt", "ods", "odp"
  ];

  const DEFAULT_MIN_SIZE_MB = 1;
  const DEDUPE_WINDOW_MS = 3_000;
  const TEMP_DOWNLOAD_EXTENSIONS = new Set(["crdownload", "tmp", "partial"]);

  /** Arabic message shown when a link serves an HTML page instead of a file. */
  const HTML_RESPONSE_MESSAGE_AR =
    "الرابط أعاد صفحة HTML بدلًا من ملف. غالبًا انتهت صلاحية الرابط أو يتطلب تسجيل الدخول. لم يُنشأ أي تنزيل.";

  /** Host error codes that mean "the server returned HTML, not a file". */
  const HTML_FAILURE_ERROR_CODES = new Set([
    "html_response",
    "not_a_file",
    "invalid_content_type",
    "content_type_html"
  ]);

  /** Dedupe cache keyed by `downloadId::cleanUrl`. */
  const urlDedupeCache = new Map();

  /** Host suffixes used for GitHub release/raw redirect targets (signed, often extensionless paths). */
  const GITHUB_REDIRECT_HOST_SUFFIXES = [".githubusercontent.com", "codeload.github.com"];

  /** Google hosts that return a file only after a confirm step. */
  const GOOGLE_DRIVE_HOSTS = new Set([
    "drive.usercontent.google.com",
    "drive.google.com",
    "docs.google.com"
  ]);

  /** MIME types that always mean "this is a file, not a page". */
  const BINARY_MIME_TYPES = new Set([
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

  /** Schemes that must never be handed to the download engine. */
  const BLOCKED_URL_SCHEMES = ["blob:", "data:", "filesystem:", "chrome-extension:"];

  function extractBasename(pathOrUrl) {
    if (!pathOrUrl || typeof pathOrUrl !== "string") return "";
    let clean = pathOrUrl.trim();
    if (clean.includes("#")) clean = clean.split("#")[0];
    if (clean.includes("?")) clean = clean.split("?")[0];
    clean = clean.replace(/\\/g, "/").replace(/\/+$/, "");
    const lastSlash = clean.lastIndexOf("/");
    const base = lastSlash >= 0 ? clean.slice(lastSlash + 1) : clean;
    try {
      return decodeURIComponent(base);
    } catch {
      return base;
    }
  }

  function extractExtension(filename) {
    const base = extractBasename(filename);
    if (!base) return "";
    const lastDot = base.lastIndexOf(".");
    if (lastDot <= 0 || lastDot === base.length - 1) return "";
    return base.slice(lastDot + 1).toLowerCase();
  }

  function parseExtensionList(input) {
    if (!input) return new Set(DEFAULT_EXTENSIONS);
    const items = Array.isArray(input)
      ? input
      : input instanceof Set
        ? Array.from(input)
        : String(input).split(",");
    const set = new Set();
    for (const raw of items) {
      const trimmed = String(raw).trim().toLowerCase().replace(/^\.+/, "");
      if (trimmed.length > 0) set.add(trimmed);
    }
    return set.size > 0 ? set : new Set(DEFAULT_EXTENSIONS);
  }

  function isTempDownloadName(filename) {
    return TEMP_DOWNLOAD_EXTENSIONS.has(extractExtension(filename));
  }

  function matchesExtension(filename, allowed = DEFAULT_EXTENSIONS) {
    const ext = extractExtension(filename);
    if (!ext) return false;
    return parseExtensionList(allowed).has(ext);
  }

  function passesSize(bytes, minSizeMb = DEFAULT_MIN_SIZE_MB) {
    if (bytes === null || bytes === undefined || typeof bytes !== "number") return true;
    if (bytes <= 0) return true;
    return bytes >= Math.max(0, minSizeMb) * 1024 * 1024;
  }

  function sanitizeUrlForLog(rawUrl) {
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
   * Signed GitHub redirect targets (objects/release-assets.githubusercontent.com,
   * codeload.github.com) that serve a file from an extensionless path.
   */
  function isGithubRedirectUrl(rawUrl) {
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
   * GitHub release downloads and source archives, including their signed CDN redirects.
   */
  function isGithubReleaseUrl(rawUrl) {
    if (!rawUrl || typeof rawUrl !== "string") return false;
    if (isGithubRedirectUrl(rawUrl)) return true;
    try {
      const parsed = new URL(rawUrl);
      const host = parsed.hostname.toLowerCase();
      if (host !== "github.com" && host !== "www.github.com") return false;
      return /^\/[^/]+\/[^/]+\/(releases\/download|archive)\//i.test(parsed.pathname);
    } catch {
      return false;
    }
  }

  /** Google Drive / Docs download URLs, including the `uc?export=download` confirm endpoint. */
  function isGoogleDriveUrl(rawUrl) {
    if (!rawUrl || typeof rawUrl !== "string") return false;
    try {
      const parsed = new URL(rawUrl);
      const host = parsed.hostname.toLowerCase();
      if (!GOOGLE_DRIVE_HOSTS.has(host)) return false;
      if (host === "drive.usercontent.google.com") return true;
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

  function normalizeMime(mime) {
    if (!mime || typeof mime !== "string") return "";
    return mime.split(";")[0].trim().toLowerCase();
  }

  function isBinaryMime(mime) {
    const type = normalizeMime(mime);
    if (!type) return false;
    if (BINARY_MIME_TYPES.has(type)) return true;
    return BINARY_MIME_PREFIXES.some((prefix) => type.startsWith(prefix));
  }

  /** True when the server answered with an HTML page instead of a file. */
  function isHtmlMime(mime) {
    const type = normalizeMime(mime);
    return type === "text/html" || type === "application/xhtml+xml";
  }

  function isBlockedUrlScheme(rawUrl) {
    if (!rawUrl || typeof rawUrl !== "string") return false;
    const lower = rawUrl.trim().toLowerCase();
    return BLOCKED_URL_SCHEMES.some((scheme) => lower.startsWith(scheme));
  }

  /**
   * Downloads that bypass the extension allowlist (and the min-size check when the
   * size is unknown): GitHub releases/archives, Drive confirm endpoints, binary MIME.
   */
  function isForceCaptureItem(item) {
    if (!item) return false;
    const urls = [item.finalUrl, item.url].filter(Boolean);
    if (urls.some(isGithubReleaseUrl)) return true;
    if (urls.some(isGoogleDriveUrl)) return true;
    return isBinaryMime(item.mime);
  }

  function buildDedupeKey(url, downloadId) {
    const cleanUrl = String(url || "").split(/[?#]/)[0];
    if (downloadId === undefined || downloadId === null || downloadId === "") {
      return cleanUrl;
    }
    return `${downloadId}::${cleanUrl}`;
  }

  /** Pure read: never records. Recording happens only after a real intercept. */
  function isUrlDeduped(url, nowMs = Date.now(), downloadId) {
    if (!url) return false;
    const lastSeen = urlDedupeCache.get(buildDedupeKey(url, downloadId));
    if (typeof lastSeen !== "number") return false;
    const age = nowMs - lastSeen;
    return age >= 0 && age < DEDUPE_WINDOW_MS;
  }

  function recordUrlDedupe(url, nowMs = Date.now(), downloadId) {
    if (!url) return;
    urlDedupeCache.set(buildDedupeKey(url, downloadId), nowMs);
    if (urlDedupeCache.size > 1000) {
      for (const [key, t] of urlDedupeCache.entries()) {
        if (typeof t !== "number" || nowMs - t > DEDUPE_WINDOW_MS) urlDedupeCache.delete(key);
      }
    }
  }

  function resetUrlDedupeCache() {
    urlDedupeCache.clear();
  }

  function resolveInterceptFilename(item, suggestedFilename, allowedExtensions) {
    const targetUrl = (item && (item.finalUrl || item.url)) || "";
    const sourceUrl = (item && item.url) || "";
    const candidates = [];

    const pushName = (raw) => {
      if (!raw || typeof raw !== "string") return;
      const base = extractBasename(raw);
      if (!base || isTempDownloadName(base)) return;
      if (!candidates.includes(base)) candidates.push(base);
    };

    // Priority: the name the browser determined (onDeterminingFilename / item.filename),
    // then the last path segment of the final URL, then of the original URL, then "download".
    if (item && item.filename) pushName(extractBasename(item.filename) || item.filename);
    if (suggestedFilename) pushName(suggestedFilename);
    pushName(targetUrl);
    if (sourceUrl && sourceUrl !== targetUrl) pushName(sourceUrl);

    for (const base of candidates) {
      if (matchesExtension(base, allowedExtensions)) return base;
    }

    if (candidates.length > 0) return candidates[0];

    const urlBase = extractBasename(targetUrl) || extractBasename(sourceUrl);
    return urlBase && !isTempDownloadName(urlBase) ? urlBase : "download";
  }

  function shouldInterceptDownload({
    item,
    settings,
    extensionId,
    suggestedFilename,
    nowMs = Date.now(),
    givenBackUrls,
    checkDedupe = false
  }) {
    const skip = (reason, extra = {}) =>
      Object.assign({ intercept: false, reason, filename: "", size: 0, targetUrl: "" }, extra);

    if (!item) return skip("invalid_item");

    if ((settings ? settings.enabled : true) === false) return skip("disabled");

    // Own download triggered by this extension, or by any other extension / restored.
    if (extensionId && item.byExtensionId === extensionId) return skip("own_download");
    if (item.byExtensionId && (!extensionId || item.byExtensionId !== extensionId)) {
      return skip("restored_download");
    }

    if (item.startTime) {
      const startMs = new Date(item.startTime).getTime();
      if (!Number.isNaN(startMs) && nowMs - startMs > 30_000) return skip("old_download");
    }

    const targetUrl = item.finalUrl || item.url || "";
    // Never hand blob:, data:, filesystem: or any other non-http(s) URL to the engine.
    if (!targetUrl || !/^https?:\/\//i.test(targetUrl)) {
      return skip("non_http_url", { targetUrl });
    }
    if (isBlockedUrlScheme(targetUrl) || isBlockedUrlScheme(item.url)) {
      return skip("non_http_url", { targetUrl });
    }

    if (givenBackUrls && (givenBackUrls.has(targetUrl) || (item.url && givenBackUrls.has(item.url)))) {
      return skip("given_back_to_browser", { targetUrl });
    }

    // Pure check — recording happens after the intercept, never while deciding.
    if (checkDedupe && isUrlDeduped(targetUrl, nowMs, item.id)) {
      return skip("url_deduped", { targetUrl });
    }

    // HTML page instead of a file: never intercept, never retry.
    if (isHtmlMime(item.mime)) {
      return skip("html_response", { targetUrl, message: HTML_RESPONSE_MESSAGE_AR });
    }

    const allowedExtensions = (settings && settings.extensions) || DEFAULT_EXTENSIONS;
    const filename = resolveInterceptFilename(item, suggestedFilename, allowedExtensions);

    // GitHub releases, Drive confirm endpoints and binary MIME do not need a listed
    // extension: those responses are files even when the URL path has no suffix.
    const forced = isForceCaptureItem(item);

    if (!forced && !matchesExtension(filename, allowedExtensions)) {
      return skip("non_listed_extension", { filename, targetUrl });
    }

    const size =
      typeof item.fileSize === "number" && item.fileSize > 0
        ? item.fileSize
        : typeof item.totalBytes === "number" && item.totalBytes > 0
          ? item.totalBytes
          : 0;

    const minSizeMb = settings ? settings.minSizeMb : DEFAULT_MIN_SIZE_MB;
    const sizeUnknown = typeof size !== "number" || size <= 0;
    // For forced captures an unknown size must never block the intercept.
    if (!(forced && sizeUnknown) &&
        !passesSize(size, minSizeMb == null ? DEFAULT_MIN_SIZE_MB : minSizeMb)) {
      return { intercept: false, reason: "too_small", filename, size, targetUrl };
    }

    return { intercept: true, reason: "allowed", filename, size, targetUrl, forced };
  }

  /**
   * Executes the capture pipeline with a browser-agnostic downloads API:
   * 1. Cancel and erase the browser download immediately (never keep both).
   *    If the item already completed, removeFile the partial file first.
   * 2. Forward the capture to the native host (10s cap in the caller).
   * 3. Give the download back to the browser ONLY on a connect/unreachable error,
   *    and never for an HTML response.
   */
  async function processDownloadCapture({
    item,
    settings,
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
        // HTML responses are terminal: show this Arabic message, never give back.
        message: decision.reason === "html_response" ? HTML_RESPONSE_MESSAGE_AR : undefined
      };
    }

    let cancelled = false;
    let removedFile = false;
    let erased = false;

    // browser downloads.cancel(id) is the very first await.
    if (downloadsApi && downloadsApi.cancel) {
      await downloadsApi.cancel(item.id);
      cancelled = true;
    }
    if (item && item.state === "complete" && downloadsApi && downloadsApi.removeFile) {
      await downloadsApi.removeFile(item.id);
      removedFile = true;
    }
    if (downloadsApi && downloadsApi.erase) {
      await downloadsApi.erase({ id: item.id });
      erased = true;
    }

    // Record dedupe only now that the download was really intercepted.
    recordUrlDedupe(decision.targetUrl, nowMs, item.id);
    if (item.url && item.url !== decision.targetUrl) recordUrlDedupe(item.url, nowMs, item.id);

    let hostResult = null;
    if (sendCaptureFn) {
      const payload = {
        url: item.url || decision.targetUrl,
        final_url: decision.targetUrl,
        filename: decision.filename,
        size: decision.size
      };
      if (referrer !== undefined) payload.referrer = referrer == null ? item.referrer || "" : referrer;
      if (cookies !== undefined) payload.cookies = Array.isArray(cookies) ? cookies : [];
      if (userAgent !== undefined) payload.user_agent = userAgent;
      if (item.mime) payload.mime = item.mime;
      hostResult = await sendCaptureFn(payload);
    }

    const isConnectError =
      hostResult &&
      !hostResult.ok &&
      (hostResult.error && (hostResult.error.code === "HOST_UNREACHABLE" ||
        hostResult.error.code === "CONNECT_ERROR"));

    let redownloaded = false;
    if (isConnectError) {
      givenBackUrls.add(decision.targetUrl);
      if (item.url) givenBackUrls.add(item.url);
      if (downloadsApi && downloadsApi.download) {
        await downloadsApi.download({
          url: decision.targetUrl,
          filename: decision.filename || undefined,
          conflictAction: "uniquify"
        });
        redownloaded = true;
      }
    }

    const htmlFailure = Boolean(
      hostResult && !hostResult.ok &&
      HTML_FAILURE_ERROR_CODES.has(String((hostResult.error && hostResult.error.code) || ""))
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

  global.SaylRules = {
    DEFAULT_EXTENSIONS,
    DEFAULT_MIN_SIZE_MB,
    DEDUPE_WINDOW_MS,
    HTML_RESPONSE_MESSAGE_AR,
    extractBasename,
    extractExtension,
    parseExtensionList,
    isTempDownloadName,
    matchesExtension,
    passesSize,
    sanitizeUrlForLog,
    isGithubRedirectUrl,
    isGithubReleaseUrl,
    isGoogleDriveUrl,
    normalizeMime,
    isBinaryMime,
    isHtmlMime,
    isBlockedUrlScheme,
    isForceCaptureItem,
    buildDedupeKey,
    recordUrlDedupe,
    isUrlDeduped,
    resetUrlDedupeCache,
    resolveInterceptFilename,
    shouldInterceptDownload,
    processDownloadCapture
  };
})(typeof globalThis !== "undefined" ? globalThis : this);

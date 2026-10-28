// SAYL Download Interception Rules - shared decision logic for the Firefox
// background script (classic script, intentionally no ES module syntax so it
// loads on every supported Gecko version). Exposes a single global, `SaylRules`,
// mirroring the pure logic used by the Chromium build.

(function (global) {
  const DEFAULT_EXTENSIONS = [
    "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "iso", "img", "dmg",
    "exe", "msi", "msix", "apk", "pkg", "bin", "dat", "mp4", "mkv", "avi",
    "mov", "wmv", "flv", "webm", "mp3", "flac", "wav", "pdf"
  ];

  const DEFAULT_MIN_SIZE_MB = 1;
  const DEDUPE_WINDOW_MS = 15_000;
  const TEMP_DOWNLOAD_EXTENSIONS = new Set(["crdownload", "tmp", "partial", ""]);

  const urlDedupeCache = new Map();

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

  function recordUrlDedupe(url, nowMs = Date.now()) {
    if (!url) return;
    urlDedupeCache.set(url, nowMs);
    if (urlDedupeCache.size > 1000) {
      for (const [u, t] of urlDedupeCache.entries()) {
        if (nowMs - t > DEDUPE_WINDOW_MS) urlDedupeCache.delete(u);
      }
    }
  }

  function isUrlDeduped(url, nowMs = Date.now()) {
    if (!url) return false;
    const lastSeen = urlDedupeCache.get(url);
    if (lastSeen && nowMs - lastSeen < DEDUPE_WINDOW_MS) return true;
    recordUrlDedupe(url, nowMs);
    return false;
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

    pushName(targetUrl);
    if (sourceUrl && sourceUrl !== targetUrl) pushName(sourceUrl);
    if (suggestedFilename) pushName(suggestedFilename);
    if (item && item.filename) pushName(item.filename);

    for (const base of candidates) {
      if (matchesExtension(base, allowedExtensions)) return base;
    }
    return candidates[0] || "download";
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
    if (!targetUrl || !/^https?:\/\//i.test(targetUrl)) {
      return skip("non_http_url", { targetUrl });
    }

    if (givenBackUrls && (givenBackUrls.has(targetUrl) || (item.url && givenBackUrls.has(item.url)))) {
      return skip("given_back_to_browser", { targetUrl });
    }

    if (checkDedupe && isUrlDeduped(targetUrl, nowMs)) {
      return skip("url_deduped", { targetUrl });
    }

    const allowedExtensions = (settings && settings.extensions) || DEFAULT_EXTENSIONS;
    const filename = resolveInterceptFilename(item, suggestedFilename, allowedExtensions);

    if (!matchesExtension(filename, allowedExtensions)) {
      return skip("non_listed_extension", { filename, targetUrl });
    }

    const size =
      typeof item.fileSize === "number" && item.fileSize > 0
        ? item.fileSize
        : typeof item.totalBytes === "number" && item.totalBytes > 0
          ? item.totalBytes
          : 0;

    const minSizeMb = settings ? settings.minSizeMb : DEFAULT_MIN_SIZE_MB;
    if (!passesSize(size, minSizeMb == null ? DEFAULT_MIN_SIZE_MB : minSizeMb)) {
      return { intercept: false, reason: "too_small", filename, size, targetUrl };
    }

    return { intercept: true, reason: "allowed", filename, size, targetUrl };
  }

  /**
   * Executes the capture pipeline with a browser-agnostic downloads API:
   * 1. Cancel and erase the browser download immediately (never keep both).
   *    If the item already completed, removeFile the partial file first.
   * 2. Forward the capture to the native host (10s cap in the caller).
   * 3. Give the download back to the browser ONLY on a connect/unreachable error.
   */
  async function processDownloadCapture({
    item,
    settings,
    extensionId,
    suggestedFilename,
    nowMs = Date.now(),
    givenBackUrls = new Set(),
    downloadsApi,
    sendCaptureFn
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
      return { intercept: false, reason: decision.reason, cancelled: false };
    }

    let cancelled = false;
    let removedFile = false;
    let erased = false;

    // chrome/browser downloads.cancel(id) is the very first await.
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

    recordUrlDedupe(decision.targetUrl, nowMs);
    if (item.url && item.url !== decision.targetUrl) recordUrlDedupe(item.url, nowMs);

    let hostResult = null;
    if (sendCaptureFn) {
      hostResult = await sendCaptureFn({
        url: item.url || decision.targetUrl,
        final_url: decision.targetUrl,
        filename: decision.filename,
        size: decision.size
      });
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

    return { intercept: true, decision, cancelled, removedFile, erased, hostResult, redownloaded };
  }

  global.SaylRules = {
    DEFAULT_EXTENSIONS,
    DEFAULT_MIN_SIZE_MB,
    extractBasename,
    extractExtension,
    matchesExtension,
    passesSize,
    sanitizeUrlForLog,
    recordUrlDedupe,
    isUrlDeduped,
    shouldInterceptDownload,
    processDownloadCapture
  };
})(typeof globalThis !== "undefined" ? globalThis : this);

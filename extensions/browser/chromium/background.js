// SAYL Download Manager - Background Service Worker (Manifest V3)
// Portions derived from AB Download Manager (https://github.com/amir1376/ab-download-manager), Apache-2.0
import {
  extractBasename,
  sanitizeUrlForLog,
  shouldInterceptDownload,
  recordUrlDedupe,
  HTML_RESPONSE_MESSAGE_AR,
  HTML_FAILURE_ERROR_CODES,
  DEFAULT_EXTENSIONS,
  DEFAULT_MIN_SIZE_MB
} from "./rules.mjs";

const HOST_NAME = "com.sayl.downloader";
const CONTEXT_MENU_ID = "sayl-download-link";
const CAPTURE_TIMEOUT_MS = 10000;
const SUCCESS_BADGE_TTL_MS = 8000;

// Set storage session access level so content script can record Alt+clicks
try {
  if (chrome.storage?.session?.setAccessLevel) {
    chrome.storage.session.setAccessLevel({
      accessLevel: "TRUSTED_AND_UNTRUSTED_CONTEXTS"
    }).catch(() => {});
  }
} catch (_) {}

// Localized strings with fallback
function msg(key) {
  try {
    return chrome.i18n.getMessage(key) || key;
  } catch {
    return key;
  }
}

// Consume lastError safely
function consumeLastError() {
  return chrome.runtime.lastError;
}

// Helper to display Chrome notifications
function notify(title, message) {
  chrome.notifications.create(
    {
      type: "basic",
      iconUrl: "icons/128x128.png",
      title: title || msg("notify_error_title"),
      message: message || ""
    },
    () => {
      consumeLastError();
    }
  );
}

// Log message to native host (fire and forget)
// detail must not contain query strings or cookie values
function sendLog(level, message, detail = "") {
  try {
    chrome.runtime.sendNativeMessage(
      HOST_NAME,
      {
        type: "Log",
        level: level || "info",
        message: String(message || ""),
        detail: detail ? String(detail) : ""
      },
      () => {
        consumeLastError();
      }
    );
  } catch (_) {}
}

// Set success badge
let badgeClearTimer = null;
function setBadgeSuccess() {
  if (badgeClearTimer) {
    clearTimeout(badgeClearTimer);
    badgeClearTimer = null;
  }
  chrome.action.setBadgeText({ text: "✓" });
  chrome.action.setBadgeBackgroundColor({ color: "#22c55e" });
  badgeClearTimer = setTimeout(() => {
    chrome.action.setBadgeText({ text: "" });
    badgeClearTimer = null;
  }, SUCCESS_BADGE_TTL_MS);
}

// Set warning/error badge and notify
function setBadgeWarning(code, message) {
  chrome.action.setBadgeText({ text: "!" });
  chrome.action.setBadgeBackgroundColor({ color: "#f59e0b" });
  const text = code ? `${code}: ${message}` : message;
  notify(msg("notify_error_title"), text);
}

// In-memory cache for settings and alt_clicks (no awaits before cancel)
let cachedSettings = {
  enabled: true,
  minSizeMb: DEFAULT_MIN_SIZE_MB,
  extensions: DEFAULT_EXTENSIONS
};
let cachedAltClicks = {};

try {
  chrome.storage?.local?.get(["enabled", "min_size_mb", "allowed_extensions"], (res) => {
    consumeLastError();
    if (res) {
      cachedSettings = {
        enabled: res.enabled !== false,
        minSizeMb:
          typeof res.min_size_mb === "number"
            ? res.min_size_mb
            : DEFAULT_MIN_SIZE_MB,
        extensions: res.allowed_extensions || DEFAULT_EXTENSIONS
      };
    }
  });
} catch (_) {}

try {
  chrome.storage?.session?.get("alt_clicks", (res) => {
    consumeLastError();
    if (res?.alt_clicks) {
      cachedAltClicks = res.alt_clicks;
    }
  });
} catch (_) {}

// Refresh cache on storage.onChanged
try {
  chrome.storage?.onChanged?.addListener((changes, areaName) => {
    if (areaName === "local") {
      if (changes.enabled) {
        cachedSettings.enabled = changes.enabled.newValue !== false;
      }
      if (changes.min_size_mb) {
        cachedSettings.minSizeMb =
          typeof changes.min_size_mb.newValue === "number"
            ? changes.min_size_mb.newValue
            : DEFAULT_MIN_SIZE_MB;
      }
      if (changes.allowed_extensions) {
        cachedSettings.extensions =
          changes.allowed_extensions.newValue || DEFAULT_EXTENSIONS;
      }
    } else if (areaName === "session") {
      if (changes.alt_clicks) {
        cachedAltClicks = changes.alt_clicks.newValue || {};
      }
    }
  });
} catch (_) {}

// Retrieve settings from local storage
async function getSettings() {
  return cachedSettings;
}

// Retrieve cookies for URL mapped to [{ name, value }]
/** Re-read the download from the Chrome downloads API (finalUrl after 302). */
async function refreshDownloadItem(item) {
  if (!item?.id) return item;
  try {
    const results = await chrome.downloads.search({ id: item.id });
    if (Array.isArray(results) && results[0]) {
      return results[0];
    }
  } catch (_) {}
  return item;
}

/**
 * Collects cookies for a capture:
 * - cookies scoped to the final URL (the exact download host, e.g. drive.usercontent.google.com)
 * - for Google downloads, the shared `.google.com` parent cookies so Drive's
 *   confirm/download handshake keeps the account session
 * Returns [{ name, value }], de-duplicated by cookie name (URL scope wins).
 */
async function getCookiesForUrl(url) {
  if (!url) return [];

  const collected = [];

  try {
    const cookies = await chrome.cookies.getAll({ url });
    for (const c of cookies || []) {
      collected.push({ name: c.name, value: c.value });
    }
  } catch (err) {
    sendLog("warn", "Failed to retrieve cookies", sanitizeUrlForLog(url));
  }

  // Google Drive confirm downloads need the account cookies from the parent domain.
  let host = "";
  try {
    host = new URL(url).hostname.toLowerCase();
  } catch (_) {
    host = "";
  }
  const isGoogleHost = host === "google.com" || host.endsWith(".google.com");
  if (isGoogleHost && chrome.cookies?.getAll) {
    try {
      const parentCookies = await chrome.cookies.getAll({ domain: ".google.com" });
      for (const c of parentCookies || []) {
        collected.push({ name: c.name, value: c.value });
      }
    } catch (_) {
      sendLog("warn", "Failed to retrieve parent google cookies", sanitizeUrlForLog(url));
    }
  }

  // De-duplicate by name, keeping the first (most specific URL-scoped) value.
  const seen = new Set();
  const result = [];
  for (const cookie of collected) {
    if (!cookie.name || seen.has(cookie.name)) continue;
    seen.add(cookie.name);
    result.push(cookie);
  }
  return result;
}

// Send Capture message to native host with 10 s timeout
function sendCaptureToHost(payload) {
  return new Promise((resolve) => {
    let settled = false;
    const timer = setTimeout(() => {
      if (settled) return;
      settled = true;
      resolve({
        ok: false,
        error: { code: "TIMEOUT", message: "SAYL host timed out after 10s" }
      });
    }, CAPTURE_TIMEOUT_MS);

    try {
      chrome.runtime.sendNativeMessage(HOST_NAME, payload, (response) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);

        const err = consumeLastError();
        if (err) {
          resolve({
            ok: false,
            error: {
              code: "HOST_UNREACHABLE",
              message: err.message || "Native host com.sayl.downloader unreachable"
            }
          });
          return;
        }

        if (!response) {
          resolve({
            ok: false,
            error: { code: "NO_RESPONSE", message: "No response from SAYL native host" }
          });
          return;
        }

        if (response.type === "Error") {
          resolve({
            ok: false,
            error: {
              code: response.code || "HOST_ERROR",
              message: response.message || "Unknown error from host"
            }
          });
          return;
        }

        if (response.type === "Accepted") {
          resolve({ ok: true, response });
          return;
        }

        resolve({
          ok: false,
          error: {
            code: "UNEXPECTED_RESPONSE",
            message: `Unexpected response type: ${response.type}`
          }
        });
      });
    } catch (ex) {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      resolve({
        ok: false,
        error: {
          code: "SEND_EXCEPTION",
          message: ex.message || "Exception sending native message"
        }
      });
    }
  });
}

const HEALTH_PROBE_INTERVAL_MS = 15_000;

// Native Health probe (never starts the desktop app)
function checkHealth() {
  chrome.runtime.sendNativeMessage(
    HOST_NAME,
    { type: "Health", version: 1 },
    (response) => {
      const err = consumeLastError();
      if (err) {
        sendLog("info", "Startup health probe: host unreachable", err.message);
        return;
      }
      if (response && response.app_connected) {
        sendLog(
          "info",
          "Startup health probe: connected",
          `app_version=${response.app_version}`
        );
      } else {
        sendLog(
          "info",
          "Startup health probe: app disconnected",
          response?.app_error || "none"
        );
      }
    }
  );
}

let healthProbeInterval = null;

function startHealthProbes() {
  checkHealth();
  if (healthProbeInterval != null) {
    return;
  }
  healthProbeInterval = setInterval(checkHealth, HEALTH_PROBE_INTERVAL_MS);
}

/** @type {Map<number, { suggestedFilename?: string, capturePostInFlight?: boolean, accepted?: boolean, handled?: boolean, cancelled?: boolean }>} */
const downloadCaptureState = new Map();

function pruneDownloadState(id) {
  setTimeout(() => downloadCaptureState.delete(id), 5 * 60 * 1000);
}

/** @type {Set<string>} URLs that failed native host capture and are given back to the browser */
const givenBackUrls = new Set();

/** Track downloads already handled to avoid double paths between onDeterminingFilename and onCreated */
const handledDownloadIds = new Set();

function markHandled(id) {
  handledDownloadIds.add(id);
  setTimeout(() => handledDownloadIds.delete(id), 5 * 60 * 1000);
}

async function handleInterceptedDownload(item, decision) {
  const id = item.id;
  let state = downloadCaptureState.get(id) || {};
  state.handled = true;
  state.cancelled = true;
  downloadCaptureState.set(id, state);

  const cleanUrl = sanitizeUrlForLog(decision.targetUrl);

  // chrome.downloads.cancel(id) is the very first await!
  try {
    await chrome.downloads.cancel(id);
  } catch (_) {}

  // If the item already completed, chrome.downloads.removeFile(id) before erase.
  try {
    if (item.state === "complete") {
      await chrome.downloads.removeFile(id);
    } else {
      const searchRes = await chrome.downloads.search({ id });
      if (searchRes?.[0]?.state === "complete") {
        await chrome.downloads.removeFile(id);
      }
    }
  } catch (_) {}

  try {
    await chrome.downloads.erase({ id });
  } catch (_) {}

  sendLog("info", `Cancelled and erased browser download ${id} immediately upon capture`, cleanUrl);

  const captureTargetUrl = decision.targetUrl || item.finalUrl || item.url || "";
  // Record the intercept keyed by download id + URL, only after the real intercept.
  recordUrlDedupe(captureTargetUrl, Date.now(), id);
  if (item.url && item.url !== captureTargetUrl) {
    recordUrlDedupe(item.url, Date.now(), id);
  }

  const cookies = await getCookiesForUrl(captureTargetUrl);
  const payload = {
    type: "Capture",
    version: 1,
    url: item.url || captureTargetUrl,
    final_url: captureTargetUrl,
    filename: decision.filename,
    referrer: item.referrer || "",
    cookies,
    user_agent: navigator.userAgent,
    size: decision.size,
    mime: item.mime || ""
  };

  sendLog(
    "info",
    "Sending capture payload to SAYL host",
    `file=${decision.filename}, size=${decision.size}`
  );

  const captureResult = await sendCaptureToHost(payload);

  state = downloadCaptureState.get(id) || {};
  state.capturePostInFlight = false;

  if (!captureResult.ok) {
    downloadCaptureState.set(id, state);
    const errCode = captureResult.error?.code || "ERROR";
    const errMsg = captureResult.error?.message || "Capture failed";
    sendLog("warn", `Capture error: ${errCode} - ${errMsg}`, cleanUrl);

    // HTML or "not a file" failure from the app: terminal. Report it in Arabic and
    // never hand the download back to the browser (that would loop forever).
    if (HTML_FAILURE_ERROR_CODES.has(errCode)) {
      setBadgeWarning(errCode, HTML_RESPONSE_MESSAGE_AR);
      return;
    }

    setBadgeWarning(errCode, errMsg);

    // Give back to browser ONLY when the host is unreachable (connect error).
    // Never on timeout or after any reply. Wait up to 10s for the reply.
    const isConnectError = errCode === "HOST_UNREACHABLE" || errCode === "CONNECT_ERROR";
    if (isConnectError) {
      givenBackUrls.add(captureTargetUrl);
      if (item.url) givenBackUrls.add(item.url);

      try {
        await chrome.downloads.download({
          url: captureTargetUrl,
          filename: decision.filename || undefined,
          conflictAction: "uniquify",
          saveAs: false
        });
        sendLog("info", `Re-downloaded in browser after host unreachable: ${cleanUrl}`);
      } catch (dlErr) {
        sendLog("warn", `Failed to re-download in browser: ${dlErr.message}`, cleanUrl);
      }
    } else {
      sendLog("info", `Not re-downloading in browser for non-connect error (${errCode})`, cleanUrl);
    }
    return;
  }

  state.accepted = true;
  downloadCaptureState.set(id, state);
  pruneDownloadState(id);

  const taskId = captureResult.response.task_id;
  sendLog("info", `تم قبول التنزيل بواسطة سيل (مهمة ${taskId})`, cleanUrl);
  setBadgeSuccess();
}

chrome.downloads.onDeterminingFilename.addListener((item, suggest) => {
  if (!item || !item.id) {
    suggest();
    return;
  }

  if (handledDownloadIds.has(item.id)) {
    return;
  }

  const suggested = item.filename || "";
  const basename = extractBasename(suggested);

  // Decide synchronously from in-memory cache — no awaits before cancel!
  const decision = shouldInterceptDownload({
    item,
    settings: cachedSettings,
    altBypasses: cachedAltClicks,
    extensionId: chrome.runtime?.id || "",
    suggestedFilename: basename || undefined,
    nowMs: Date.now(),
    givenBackUrls,
    checkDedupe: true
  });

  if (!decision.intercept) {
    // HTML page instead of a file: tell the user in Arabic, never create a task,
    // and never hand it to the host (no loop).
    if (decision.reason === "html_response") {
      markHandled(item.id);
      suggest({ filename: item.filename || suggested });
      notify(msg("notify_error_title"), HTML_RESPONSE_MESSAGE_AR);
      sendLog(
        "warn",
        "Download skipped: html_response",
        sanitizeUrlForLog(decision.targetUrl)
      );
      return;
    }

    // Call suggest() ONLY for downloads we don't intercept!
    markHandled(item.id);
    suggest({ filename: item.filename || suggested });
    if (decision.reason === "given_back_to_browser") {
      givenBackUrls.delete(decision.targetUrl);
      if (item.url) givenBackUrls.delete(item.url);
    }
    sendLog("info", `Download skipped: ${decision.reason}`, sanitizeUrlForLog(decision.targetUrl));
    return;
  }

  // Intercepting:
  // DON'T call suggest()!
  markHandled(item.id);

  // Asynchronously cancel, erase, and forward to host
  void handleInterceptedDownload(item, decision);

  // return true to tell Chrome filename determination is handled asynchronously
  return true;
});

// Chromium decides only in onDeterminingFilename: a fallback decision here would
// race it (Chrome fires onCreated first) and the dedupe must not consume the item.
// onCreated is used only where onDeterminingFilename does not exist (Firefox).
if (!chrome.downloads?.onDeterminingFilename) {
  chrome.downloads?.onCreated?.addListener(async (item) => {
    if (!item || !item.id) return;

    if (handledDownloadIds.has(item.id) || downloadCaptureState.get(item.id)?.handled) {
      return;
    }

    const decision = shouldInterceptDownload({
      item,
      settings: cachedSettings,
      altBypasses: cachedAltClicks,
      extensionId: chrome.runtime?.id || "",
      suggestedFilename: extractBasename(item.filename),
      nowMs: Date.now(),
      givenBackUrls,
      checkDedupe: true
    });

    if (decision.reason === "html_response") {
      markHandled(item.id);
      notify(msg("notify_error_title"), HTML_RESPONSE_MESSAGE_AR);
      sendLog("warn", "Download skipped: html_response", sanitizeUrlForLog(decision.targetUrl));
      return;
    }

    if (decision.intercept) {
      markHandled(item.id);
      await handleInterceptedDownload(item, decision);
    }
  });
}

// Setup context menu on installation
chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.create(
    {
      id: CONTEXT_MENU_ID,
      title: msg("context_menu_download"),
      contexts: ["link"]
    },
    () => {
      consumeLastError();
    }
  );
  startHealthProbes();
});

chrome.runtime.onStartup.addListener(() => {
  startHealthProbes();
});

startHealthProbes();

// Handle context menu clicks
chrome.contextMenus.onClicked.addListener(async (info) => {
  if (info.menuItemId !== CONTEXT_MENU_ID || !info.linkUrl) return;

  if (!/^https?:\/\//i.test(info.linkUrl)) {
    notify(msg("notify_error_title"), "يمكن إرسال روابط http(s) فقط إلى سيل.");
    return;
  }

  const targetUrl = info.linkUrl;
  const cleanUrl = sanitizeUrlForLog(targetUrl);
  const filename = extractBasename(targetUrl) || "download";
  const cookies = await getCookiesForUrl(targetUrl);

  const payload = {
    type: "Capture",
    version: 1,
    url: targetUrl,
    final_url: targetUrl,
    filename,
    referrer: info.pageUrl || "",
    cookies,
    user_agent: navigator.userAgent,
    size: 0,
    mime: ""
  };

  sendLog("info", "Context menu capture triggered", cleanUrl);

  const result = await sendCaptureToHost(payload);
  if (!result.ok) {
    sendLog(
      "warn",
      `Context menu capture error: ${result.error.code} - ${result.error.message}`,
      cleanUrl
    );
    setBadgeWarning(result.error.code, result.error.message);
    return;
  }

  const taskId = result.response.task_id;
  sendLog("info", `تم قبول التنزيل بواسطة سيل (مهمة ${taskId})`, cleanUrl);
  setBadgeSuccess();
  notify(msg("notify_accepted_title"), `مهمة: ${taskId}`);
});

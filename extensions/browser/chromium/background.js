// SAYL Download Manager - Background Service Worker (Manifest V3)
import {
  extractBasename,
  sanitizeUrlForLog,
  shouldInterceptDownload,
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

// Retrieve settings from local storage
async function getSettings() {
  try {
    const res = await chrome.storage.local.get([
      "enabled",
      "min_size_mb",
      "allowed_extensions"
    ]);
    return {
      enabled: res.enabled !== false,
      minSizeMb:
        typeof res.min_size_mb === "number"
          ? res.min_size_mb
          : DEFAULT_MIN_SIZE_MB,
      extensions: res.allowed_extensions || DEFAULT_EXTENSIONS
    };
  } catch {
    return {
      enabled: true,
      minSizeMb: DEFAULT_MIN_SIZE_MB,
      extensions: DEFAULT_EXTENSIONS
    };
  }
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

async function getCookiesForUrl(url) {
  if (!url) return [];
  try {
    const cookies = await chrome.cookies.getAll({ url });
    return (cookies || []).map((c) => ({
      name: c.name,
      value: c.value
    }));
  } catch (err) {
    sendLog("warn", "Failed to retrieve cookies", sanitizeUrlForLog(url));
    return [];
  }
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

/** @type {Map<number, { suggestedFilename?: string, capturePostInFlight?: boolean, accepted?: boolean }>} */
const downloadCaptureState = new Map();

function pruneDownloadState(id) {
  setTimeout(() => downloadCaptureState.delete(id), 5 * 60 * 1000);
}

async function cancelBrowserDownload(item, taskId, cleanUrl) {
  try {
    await chrome.downloads.cancel(item.id);
    await new Promise((resolve) => setTimeout(resolve, 150));
    const [curr] = await chrome.downloads.search({ id: item.id });
    if (curr) {
      try {
        await chrome.downloads.removeFile(item.id);
      } catch (_) {}
    }
    await chrome.downloads.erase({ id: item.id });
    sendLog("info", `Cancelled browser download ${item.id}`, `task=${taskId}`);
  } catch (cancelErr) {
    sendLog(
      "warn",
      `Failed to cancel/erase browser download: ${cancelErr.message}`,
      cleanUrl
    );
  }
}

async function evaluateAndCaptureDownload(item, suggestedFilename) {
  if (!item || !item.id) return;

  const id = item.id;
  let state = downloadCaptureState.get(id) || {};
  if (state.accepted || state.capturePostInFlight) return;

  if (suggestedFilename) {
    state.suggestedFilename = suggestedFilename;
  }
  downloadCaptureState.set(id, state);

  const effectiveSuggested = state.suggestedFilename || suggestedFilename;

  try {
    const freshItem = await refreshDownloadItem(item);
    const targetUrl = freshItem.finalUrl || freshItem.url || "";
    const cleanUrl = sanitizeUrlForLog(targetUrl);

    const settings = await getSettings();

    let altBypasses = {};
    try {
      const sessionRes = await chrome.storage.session.get("alt_clicks");
      altBypasses = (sessionRes && sessionRes.alt_clicks) || {};
    } catch (_) {}

    const decision = shouldInterceptDownload({
      item: freshItem,
      settings,
      altBypasses,
      extensionId: chrome.runtime.id,
      suggestedFilename: effectiveSuggested,
      nowMs: Date.now()
    });

    if (!decision.intercept) {
      sendLog("info", `Download skipped: ${decision.reason}`, cleanUrl);
      return;
    }

    state = downloadCaptureState.get(id) || {};
    if (state.accepted || state.capturePostInFlight) return;

    sendLog("info", "Download matched rules", cleanUrl);

    const captureTargetUrl = decision.targetUrl || targetUrl;
    const cookies = await getCookiesForUrl(captureTargetUrl);
    state.capturePostInFlight = true;
    downloadCaptureState.set(id, state);

    const payload = {
      type: "Capture",
      version: 1,
      url: freshItem.url || captureTargetUrl,
      final_url: captureTargetUrl,
      filename: decision.filename,
      referrer: freshItem.referrer || "",
      cookies,
      user_agent: navigator.userAgent,
      size: decision.size,
      mime: freshItem.mime || ""
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
      const errCode = captureResult.error.code;
      const errMsg = captureResult.error.message;
      sendLog("warn", `Fallback error: ${errCode} - ${errMsg}`, cleanUrl);
      setBadgeWarning(errCode, errMsg);
      return;
    }

    state.accepted = true;
    downloadCaptureState.set(id, state);
    pruneDownloadState(id);

    const taskId = captureResult.response.task_id;
    sendLog("info", `Download accepted by SAYL (task ${taskId})`, cleanUrl);
    setBadgeSuccess();

    await cancelBrowserDownload(freshItem, taskId, cleanUrl);
  } catch (err) {
    state = downloadCaptureState.get(id) || {};
    if (!state.accepted) {
      state.capturePostInFlight = false;
      downloadCaptureState.set(id, state);
    }
    throw err;
  }
}

chrome.downloads.onDeterminingFilename.addListener((item, suggest) => {
  const suggested = item.filename || "";
  suggest({ filename: suggested });
  const basename = extractBasename(suggested);
  void evaluateAndCaptureDownload(item, basename || undefined);
});

chrome.downloads.onCreated.addListener(async (item) => {
  if (!item || !item.id) return;
  sendLog("info", "Download created", sanitizeUrlForLog(item.finalUrl || item.url || ""));
  await evaluateAndCaptureDownload(item, undefined);
});

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
    notify(msg("notify_error_title"), "Only http(s) links can be sent to SAYL.");
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
  sendLog("info", `Context menu capture accepted (task ${taskId})`, cleanUrl);
  setBadgeSuccess();
  notify(msg("notify_accepted_title"), `Task: ${taskId}`);
});

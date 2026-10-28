// SAYL Download Manager - Background Script (Firefox Manifest V3)
//
// Intercepts downloads exactly like the Chromium build: cancel the browser
// download first, forward the capture to the native host, and hand the download
// back to Firefox ONLY when the host is unreachable. All user-facing text is
// Arabic. Every browser API call is guarded so a missing or unsupported API
// degrades gracefully instead of throwing.

const HOST_NAME = "com.sayl.downloader";
const CONTEXT_MENU_ID = "sayl-download-link";
const CAPTURE_TIMEOUT_MS = 10000;
const HEALTH_PROBE_INTERVAL_MS = 15000;

const rules = globalThis.SaylRules || {};
const {
  DEFAULT_EXTENSIONS = [],
  DEFAULT_MIN_SIZE_MB = 1,
  extractBasename = () => "",
  sanitizeUrlForLog = () => "",
  recordUrlDedupe = () => {},
  shouldInterceptDownload = () => ({ intercept: false, reason: "rules_missing" })
} = rules;

// Localized strings with a literal fallback so the script still behaves when
// browser.i18n is unavailable (unit/stub contexts).
function msg(key) {
  try {
    return browser.i18n.getMessage(key) || key;
  } catch {
    return key;
  }
}

// Helper to display notifications using the promise-based browser API.
function notify(title, message) {
  try {
    const p = browser.notifications.create({
      type: "basic",
      iconUrl: "icons/128x128.png",
      title: title,
      message: message
    });
    if (p && typeof p.catch === "function") {
      p.catch(() => {
        // Ignored if notifications are denied
      });
    }
  } catch {
    // Ignored if notifications fail
  }
}

// In-memory cache for settings so the interception decision stays synchronous.
let cachedSettings = {
  enabled: true,
  minSizeMb: DEFAULT_MIN_SIZE_MB,
  extensions: DEFAULT_EXTENSIONS
};

function applySettings(res) {
  if (!res) return;
  cachedSettings = {
    enabled: res.enabled !== false,
    minSizeMb:
      typeof res.min_size_mb === "number" ? res.min_size_mb : DEFAULT_MIN_SIZE_MB,
    extensions: res.allowed_extensions || DEFAULT_EXTENSIONS
  };
}

// Seed the settings cache once, defensively.
try {
  if (browser.storage && browser.storage.local) {
    browser.storage.local
      .get(["enabled", "min_size_mb", "allowed_extensions"])
      .then(applySettings)
      .catch(() => {});
  }
} catch {
  // ignore
}

// Keep the cache fresh while the event page lives.
try {
  if (browser.storage && browser.storage.onChanged) {
    browser.storage.onChanged.addListener((changes, areaName) => {
      if (areaName !== "local") return;
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
    });
  }
} catch {
  // ignore
}

async function getCookiesForUrl(url) {
  if (!url) return [];
  try {
    if (!browser.cookies || !browser.cookies.getAll) return [];
    const cookies = await browser.cookies.getAll({ url });
    return (cookies || []).map((c) => ({ name: c.name, value: c.value }));
  } catch {
    return [];
  }
}

// Send a Capture message to the native host with a 10 s cap.
function sendCaptureToHost(payload) {
  return new Promise((resolve) => {
    let settled = false;
    const timer = setTimeout(() => {
      if (settled) return;
      settled = true;
      resolve({ ok: false, error: { code: "TIMEOUT", message: "انتهت مهلة الانتظار" } });
    }, CAPTURE_TIMEOUT_MS);

    const finish = (result) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      resolve(result);
    };

    try {
      Promise.resolve(browser.runtime.sendNativeMessage(HOST_NAME, payload))
        .then((response) => {
          if (!response) {
            finish({
              ok: false,
              error: { code: "NO_RESPONSE", message: "لا توجد استجابة من سيل" }
            });
            return;
          }
          if (response.type === "Accepted") {
            finish({ ok: true, response });
            return;
          }
          if (response.type === "Error") {
            finish({
              ok: false,
              error: {
                code: response.code || "HOST_ERROR",
                message: response.message || "رفض سيل طلب التنزيل"
              }
            });
            return;
          }
          finish({
            ok: false,
            error: { code: "UNEXPECTED_RESPONSE", message: "استجابة غير متوقعة من سيل" }
          });
        })
        .catch((err) => {
          // The host is missing or refused the connection: let Firefox keep it.
          finish({
            ok: false,
            error: {
              code: "HOST_UNREACHABLE",
              message: (err && err.message) || "تعذر الوصول إلى سيل"
            }
          });
        });
    } catch (ex) {
      finish({
        ok: false,
        error: {
          code: "HOST_UNREACHABLE",
          message: (ex && ex.message) || "تعذر الوصول إلى سيل"
        }
      });
    }
  });
}

const downloadsApi = browser.downloads || null;

// Downloads already handled, so onCreated cannot double-handle an id.
const handledDownloadIds = new Set();

function markHandled(id) {
  handledDownloadIds.add(id);
  setTimeout(() => handledDownloadIds.delete(id), 5 * 60 * 1000);
}

/** URLs handed back to Firefox once, so they are never re-intercepted. */
const givenBackUrls = new Set();

async function handleInterceptedDownload(item, decision) {
  const id = item.id;
  const cleanUrl = sanitizeUrlForLog(decision.targetUrl);

  // Cancel the browser download first, before any other await.
  try {
    if (downloadsApi && downloadsApi.cancel) await downloadsApi.cancel(id);
  } catch {
    // ignore
  }

  // If the item already completed, delete the partial file before erasing.
  try {
    let state = item.state;
    if (downloadsApi && downloadsApi.search) {
      const found = await downloadsApi.search({ id });
      if (found && found[0] && found[0].state) state = found[0].state;
    }
    if (state === "complete" && downloadsApi && downloadsApi.removeFile) {
      await downloadsApi.removeFile(id);
    }
  } catch {
    // ignore
  }

  try {
    if (downloadsApi && downloadsApi.erase) await downloadsApi.erase({ id });
  } catch {
    // ignore
  }

  const captureTargetUrl = decision.targetUrl || item.finalUrl || item.url || "";
  recordUrlDedupe(captureTargetUrl);
  if (item.url && item.url !== captureTargetUrl) recordUrlDedupe(item.url);

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

  const captureResult = await sendCaptureToHost(payload);

  if (!captureResult.ok) {
    const errCode = (captureResult.error && captureResult.error.code) || "ERROR";

    // Give the download back to Firefox ONLY when the host is unreachable.
    // Never on timeout and never after any other reply.
    const isConnectError = errCode === "HOST_UNREACHABLE" || errCode === "CONNECT_ERROR";
    if (isConnectError) {
      givenBackUrls.add(captureTargetUrl);
      if (item.url) givenBackUrls.add(item.url);
      try {
        if (downloadsApi && downloadsApi.download) {
          await downloadsApi.download({
            url: captureTargetUrl,
            filename: decision.filename || undefined,
            conflictAction: "uniquify",
            saveAs: false
          });
        }
      } catch {
        // ignore
      }
    }
    return;
  }

  const taskId = captureResult.response && captureResult.response.task_id;
  if (taskId) {
    notify(msg("notify_accepted_title"), `مهمة: ${taskId}`);
  }
}

// Decide synchronously from the in-memory cache — no awaits before cancel.
function handleCapture(item, suggestedFilename) {
  if (!item || !item.id) return false;

  if (handledDownloadIds.has(item.id)) return false;

  const decision = shouldInterceptDownload({
    item,
    settings: cachedSettings,
    extensionId: (browser.runtime && browser.runtime.id) || "",
    suggestedFilename: suggestedFilename || undefined,
    nowMs: Date.now(),
    givenBackUrls,
    checkDedupe: true
  });

  if (!decision.intercept) {
    if (decision.reason === "given_back_to_browser") {
      givenBackUrls.delete(decision.targetUrl);
      if (item.url) givenBackUrls.delete(item.url);
    }
    return false;
  }

  markHandled(item.id);
  void handleInterceptedDownload(item, decision);
  return true;
}

// Firefox has no onDeterminingFilename; when it exists (feature-detected) use it,
// otherwise decide in onCreated. Either way cancel happens before any other await.
try {
  if (downloadsApi && downloadsApi.onDeterminingFilename) {
    downloadsApi.onDeterminingFilename.addListener((item, suggest) => {
      if (!item || !item.id) {
        suggest();
        return;
      }
      if (handledDownloadIds.has(item.id)) return;

      const intercepting = handleCapture(item, extractBasename(item.filename));
      if (intercepting) {
        // Do not call suggest() when we handle the filename ourselves.
        return true;
      }
      markHandled(item.id);
      suggest({ filename: item.filename || item.filename || "" });
      return;
    });
  }
} catch {
  // ignore
}

try {
  if (downloadsApi && downloadsApi.onCreated) {
    downloadsApi.onCreated.addListener(async (item) => {
      if (!item || !item.id) return;
      if (handledDownloadIds.has(item.id)) return;

      if (downloadsApi.onDeterminingFilename) {
        // Give the synchronous determining listener a tick to claim the item.
        await new Promise((r) => setTimeout(r, 50));
        if (handledDownloadIds.has(item.id)) return;
      }

      handleCapture(item, extractBasename(item.filename));
    });
  }
} catch {
  // ignore
}

// Silent native-host probe (never starts the desktop app, never surfaces errors).
function checkHealth() {
  try {
    if (!browser.runtime || !browser.runtime.sendNativeMessage) return;
    Promise.resolve(browser.runtime.sendNativeMessage(HOST_NAME, { type: "Health", version: 1 }))
      .then(() => {})
      .catch(() => {
        // Expected when SAYL is not installed or not running.
      });
  } catch {
    // ignore
  }
}

let healthProbeInterval = null;

function startHealthProbes() {
  checkHealth();
  if (healthProbeInterval != null) return;
  healthProbeInterval = setInterval(checkHealth, HEALTH_PROBE_INTERVAL_MS);
}

// Register link context menu
function registerContextMenu() {
  try {
    if (!browser.contextMenus || !browser.contextMenus.create) return;
    browser.contextMenus.create(
      {
        id: CONTEXT_MENU_ID,
        title: msg("context_menu_download"),
        contexts: ["link"]
      },
      () => {
        if (browser.runtime && browser.runtime.lastError) {
          // Swallow expected duplicate registration errors
        }
      }
    );
  } catch {
    // Swallow expected duplicate registration errors
  }
}

try {
  browser.runtime.onInstalled.addListener(() => {
    registerContextMenu();
    startHealthProbes();
  });
} catch {
  // ignore
}

try {
  browser.runtime.onStartup.addListener(() => {
    startHealthProbes();
  });
} catch {
  // ignore
}

startHealthProbes();

// Handle context menu clicks on links
try {
  browser.contextMenus.onClicked.addListener(async (info) => {
    if (info.menuItemId !== CONTEXT_MENU_ID || !info.linkUrl) return;
    if (!/^https?:\/\//i.test(info.linkUrl)) {
      notify(msg("notify_error_title"), "يمكن إرسال روابط http(s) فقط إلى سيل.");
      return;
    }

    const targetUrl = info.linkUrl;
    const filename = extractBasename(targetUrl) || "download";
    const cookies = await getCookiesForUrl(targetUrl);

    const result = await sendCaptureToHost({
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
    });

    if (!result.ok) {
      notify(msg("notify_error_title"), "تعذر إرسال التنزيل إلى سيل. جرّب مرة أخرى.");
      return;
    }

    const taskId = result.response && result.response.task_id;
    notify(msg("notify_accepted_title"), taskId ? `مهمة: ${taskId}` : msg("notify_accepted_title"));
  });
} catch {
  // ignore
}

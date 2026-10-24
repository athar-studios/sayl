// SAYL Download Manager - Background Service Worker (Manifest V3)

const HOST_NAME = "com.sayl.downloader";
const CONTEXT_MENU_ID = "sayl-download-link";
const FALLBACK_GUARD_HEADER = "x-sayl-fallback";

// Localized strings with a literal fallback so the worker still behaves when
// chrome.i18n is unavailable (unit/stub contexts).
function msg(key) {
  try {
    return chrome.i18n.getMessage(key) || key;
  } catch {
    return key;
  }
}

// Helper to display Chrome notifications with safe lastError consumption
function notify(title, message) {
  chrome.notifications.create({
    type: "basic",
    iconUrl: "icons/128x128.png",
    title: title,
    message: message
  }, () => {
    // Read and ignore lastError if notification permissions are denied
    const _ = chrome.runtime.lastError;
  });
}

// Swallow lastError for calls whose failure is an expected state.
function consumeLastError() {
  return chrome.runtime.lastError;
}

// Ping host at startup to verify native messaging connectivity
function pingHost() {
  chrome.runtime.sendNativeMessage(HOST_NAME, { type: "Ping", version: 1 }, (response) => {
    const err = consumeLastError();
    if (err) {
      // Expected when SAYL is not installed or not running.
      return;
    }
    if (response && response.type === "Pong") {
      console.log(`SAYL host reachable: v${response.version}`);
    }
  });
}

// Register link context menu on installation
chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.create({
    id: CONTEXT_MENU_ID,
    title: msg("context_menu_download"),
    contexts: ["link"]
  }, () => {
    consumeLastError();
  });
  pingHost();
});

chrome.runtime.onStartup.addListener(() => {
  pingHost();
});

// Initial probe when worker starts
pingHost();

// Handle context menu clicks on links
chrome.contextMenus.onClicked.addListener((info) => {
  if (info.menuItemId !== CONTEXT_MENU_ID || !info.linkUrl) {
    return;
  }
  if (!/^https?:\/\//i.test(info.linkUrl)) {
    notify(msg("notify_error_title"), "Only http(s) links can be sent to SAYL.");
    return;
  }

  chrome.runtime.sendNativeMessage(HOST_NAME, {
    type: "Download",
    url: info.linkUrl,
    referrer: info.pageUrl || null
  }, (response) => {
    const err = consumeLastError();
    if (err) {
      // SAYL is not installed or not running: leave the page untouched so the
      // user can use the browser's own download instead.
      notify(msg("notify_fallback_title"), msg("notify_fallback_body"));
      return;
    }
    if (!response) {
      notify(msg("notify_error_title"), "No response received from SAYL.");
      return;
    }
    if (response.type === "Error") {
      notify(msg("notify_error_title"), `${response.code}: ${response.message}`);
    } else if (response.type === "Accepted") {
      notify(msg("notify_accepted_title"), `Task: ${response.task_id}`);
    }
  });
});

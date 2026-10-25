// SAYL Download Manager - Background Script (Firefox Manifest V3)

const HOST_NAME = "com.sayl.downloader";
const CONTEXT_MENU_ID = "sayl-download-link";

// Localized strings with a literal fallback so the script still behaves when
// browser.i18n is unavailable (unit/stub contexts).
function msg(key) {
  try {
    return browser.i18n.getMessage(key) || key;
  } catch {
    return key;
  }
}

// Helper to display notifications using promise-based browser API
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

// Ping host at startup to verify native messaging connectivity
async function pingHost() {
  try {
    const response = await browser.runtime.sendNativeMessage(HOST_NAME, {
      type: "Ping",
      version: 1
    });
    if (response && response.type === "Pong") {
      console.log(`SAYL host reachable: v${response.version}`);
    }
  } catch {
    // Expected when SAYL is not installed or not running.
  }
}

// Register link context menu
function registerContextMenu() {
  try {
    browser.contextMenus.create({
      id: CONTEXT_MENU_ID,
      title: msg("context_menu_download"),
      contexts: ["link"]
    }, () => {
      if (browser.runtime && browser.runtime.lastError) {
        // Swallow expected duplicate registration errors
      }
    });
  } catch {
    // Swallow expected duplicate registration errors
  }
}

// Register link context menu on installation
browser.runtime.onInstalled.addListener(() => {
  registerContextMenu();
  pingHost();
});

browser.runtime.onStartup.addListener(() => {
  pingHost();
});

// Initial probe when script runs
pingHost();

// Handle context menu clicks on links
browser.contextMenus.onClicked.addListener(async (info) => {
  if (info.menuItemId !== CONTEXT_MENU_ID || !info.linkUrl) {
    return;
  }
  if (!/^https?:\/\//i.test(info.linkUrl)) {
    notify(msg("notify_error_title"), "Only http(s) links can be sent to SAYL.");
    return;
  }

  try {
    const response = await browser.runtime.sendNativeMessage(HOST_NAME, {
      type: "Download",
      url: info.linkUrl,
      referrer: info.pageUrl || null
    });
    if (!response) {
      notify(msg("notify_error_title"), "No response received from SAYL.");
      return;
    }
    if (response.type === "Error") {
      notify(msg("notify_error_title"), `${response.code}: ${response.message}`);
    } else if (response.type === "Accepted") {
      notify(msg("notify_accepted_title"), `Task: ${response.task_id}`);
    }
  } catch (err) {
    // SAYL is not installed or not running: notify fallback
    notify(msg("notify_fallback_title"), msg("notify_fallback_body"));
  }
});

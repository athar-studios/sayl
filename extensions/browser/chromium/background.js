// SAYL Download Manager - Background Service Worker (Manifest V3)

const HOST_NAME = "com.sayl.downloader";
const CONTEXT_MENU_ID = "sayl-download-link";

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

// Ping host at startup to verify native messaging connectivity
function pingHost() {
  chrome.runtime.sendNativeMessage(HOST_NAME, { type: "Ping", version: 1 }, (response) => {
    const err = chrome.runtime.lastError;
    if (err) {
      console.warn("SAYL host ping failed (host may not be registered yet):", err.message);
      return;
    }
    if (response && response.type === "Pong") {
      console.log(`SAYL host reachable: v${response.version} (${response.app_version})`);
    }
  });
}

// Register link context menu on installation
chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.create({
    id: CONTEXT_MENU_ID,
    title: "تنزيل باستخدام سيل",
    contexts: ["link"]
  }, () => {
    const _ = chrome.runtime.lastError;
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

  const payload = {
    type: "Download",
    url: info.linkUrl,
    referrer: info.pageUrl || null
  };

  chrome.runtime.sendNativeMessage(HOST_NAME, payload, (response) => {
    const err = chrome.runtime.lastError;
    if (err) {
      notify("خطأ في الاتصال / Connection Error", err.message || "Could not communicate with SAYL host");
      return;
    }

    if (!response) {
      notify("خطأ / Error", "لم يتم تلقي رد من مضيف سيل / No response received from SAYL host");
      return;
    }

    if (response.type === "Error") {
      notify("فشل التنزيل / Download Failed", `${response.code}: ${response.message}`);
    } else if (response.type === "Accepted") {
      notify("تم قبول التنزيل / Download Accepted", `المهمة / Task: ${response.task_id}`);
    }
  });
});

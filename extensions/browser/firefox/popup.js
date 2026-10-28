// SAYL Download Manager - Popup Script (Firefox)
//
// The popup shows one on/off toggle and one short Arabic status word. It
// reconnects silently: the status probe runs on open and, while the host or app
// is unavailable, retries on a short interval for as long as the popup is open.
// There are no connection buttons and no technical text.

const HOST_NAME = "com.sayl.downloader";
const RETRY_INTERVAL_MS = 2000;

function msg(key) {
  try {
    return browser.i18n.getMessage(key) || key;
  } catch {
    return key;
  }
}

document.addEventListener("DOMContentLoaded", () => {
  const titleMain = document.getElementById("title-main");
  const settingEnabled = document.getElementById("setting-enabled");
  const statusDot = document.getElementById("status-dot");
  const statusText = document.getElementById("status-text");

  if (titleMain) titleMain.textContent = msg("extension_name");

  // Load enabled state, defensively.
  try {
    if (browser.storage?.local) {
      browser.storage.local
        .get("enabled")
        .then((items) => {
          if (settingEnabled) {
            settingEnabled.checked = !items || items.enabled !== false;
          }
        })
        .catch(() => {});
    }
  } catch (_) {}

  if (settingEnabled) {
    settingEnabled.addEventListener("change", (e) => {
      try {
        if (browser.storage?.local) {
          browser.storage.local.set({ enabled: e.target.checked }).catch(() => {});
        }
      } catch (_) {}
    });
  }

  // Three simple states only.
  function renderState(level) {
    const map = {
      connected: { cls: "success", key: "status_connected" },
      not_installed: { cls: "error", key: "status_not_installed" },
      not_connected: { cls: "pending", key: "status_app_disconnected" }
    };
    const state = map[level] || map.not_connected;
    if (statusDot) statusDot.className = `dot ${state.cls}`;
    if (statusText) {
      statusText.className = `status-text ${state.cls}`;
      statusText.textContent = msg(state.key);
    }
  }

  renderState("not_connected");

  let retryTimer = null;

  function stopRetrying() {
    if (retryTimer) {
      clearInterval(retryTimer);
      retryTimer = null;
    }
  }

  function scheduleRetry() {
    if (retryTimer) return;
    retryTimer = setInterval(probeHost, RETRY_INTERVAL_MS);
  }

  function probeHost() {
    let promise;
    try {
      promise = browser.runtime.sendNativeMessage(HOST_NAME, { type: "Health", version: 1 });
    } catch (_) {
      renderState("not_installed");
      scheduleRetry();
      return;
    }

    Promise.resolve(promise)
      .then((response) => {
        if (response && response.app_connected === true) {
          renderState("connected");
          stopRetrying();
          return;
        }
        renderState("not_connected");
        scheduleRetry();
      })
      .catch(() => {
        // Host is not installed / unreachable.
        renderState("not_installed");
        scheduleRetry();
      });
  }

  probeHost();

  window.addEventListener("unload", stopRetrying);
});

// SAYL Download Manager - Popup Script
import {
  DEFAULT_EXTENSIONS,
  DEFAULT_MIN_SIZE_MB
} from "./rules.mjs";

const HOST_NAME = "com.sayl.downloader";

function msg(key) {
  try {
    return chrome.i18n.getMessage(key) || key;
  } catch {
    return key;
  }
}

document.addEventListener("DOMContentLoaded", () => {
  // Localize static UI
  const titleMain = document.getElementById("title-main");
  const settingsTitle = document.getElementById("settings-title");
  const lblEnabled = document.getElementById("lbl-enabled");
  const lblMinSize = document.getElementById("lbl-min-size");
  const lblExtensions = document.getElementById("lbl-extensions");
  const testBtn = document.getElementById("test-btn");
  const savedNotice = document.getElementById("saved-notice");

  if (titleMain) titleMain.textContent = msg("extension_name");
  if (settingsTitle) settingsTitle.textContent = msg("settings_title");
  if (lblEnabled) lblEnabled.textContent = msg("setting_enabled");
  if (lblMinSize) lblMinSize.textContent = msg("setting_min_size");
  if (lblExtensions) lblExtensions.textContent = msg("setting_extensions");
  if (testBtn) testBtn.textContent = msg("btn_test");
  if (savedNotice) savedNotice.textContent = msg("setting_saved");

  // Health elements
  const statusDot = document.getElementById("status-dot");
  const statusText = document.getElementById("status-text");
  const statusDetails = document.getElementById("status-details");

  // Settings elements
  const settingEnabled = document.getElementById("setting-enabled");
  const settingMinSize = document.getElementById("setting-min-size");
  const settingExtensions = document.getElementById("setting-extensions");

  function runHealthCheck(isTest = false) {
    statusDot.className = "dot pending";
    statusText.textContent = msg("status_checking");
    if (!isTest) {
      statusDetails.classList.add("hidden");
    }

    try {
      chrome.runtime.sendNativeMessage(
        HOST_NAME,
        { type: "Health", version: 1 },
        (response) => {
          const err = chrome.runtime.lastError;
          if (err) {
            statusDot.className = "dot error";
            statusText.textContent = msg("status_host_disconnected");
            statusDetails.textContent = isTest
              ? JSON.stringify({ error: err.message || "Host unreachable" }, null, 2)
              : (err.message || "Native host com.sayl.downloader unreachable");
            statusDetails.classList.remove("hidden");
            return;
          }

          if (!response) {
            statusDot.className = "dot error";
            statusText.textContent = msg("status_host_disconnected");
            statusDetails.textContent = isTest
              ? JSON.stringify({ error: "Empty response from host" }, null, 2)
              : "No response from SAYL native host";
            statusDetails.classList.remove("hidden");
            return;
          }

          // Green only when app_connected is true; otherwise exact app_error
          if (response.app_connected === true) {
            statusDot.className = "dot success";
            statusText.textContent = msg("status_connected");
            statusDetails.textContent = isTest
              ? JSON.stringify(response, null, 2)
              : `SAYL v${response.app_version || "unknown"} (Protocol v${response.app_protocol || 1})`;
            statusDetails.classList.remove("hidden");
          } else {
            statusDot.className = "dot error";
            statusText.textContent = msg("status_app_disconnected");
            statusDetails.textContent = isTest
              ? JSON.stringify(response, null, 2)
              : (response.app_error || "SAYL desktop application is not running");
            statusDetails.classList.remove("hidden");
          }
        }
      );
    } catch (ex) {
      statusDot.className = "dot error";
      statusText.textContent = msg("status_host_disconnected");
      statusDetails.textContent = isTest
        ? JSON.stringify({ exception: ex.message }, null, 2)
        : (ex.message || "Failed to invoke native messaging");
      statusDetails.classList.remove("hidden");
    }
  }

  // Handle test button click
  if (testBtn) {
    testBtn.addEventListener("click", () => {
      runHealthCheck(true);
    });
  }

  // Load persisted settings
  chrome.storage.local.get(
    ["enabled", "min_size_mb", "allowed_extensions"],
    (items) => {
      if (chrome.runtime.lastError) return;
      settingEnabled.checked = items.enabled !== false;
      settingMinSize.value =
        typeof items.min_size_mb === "number"
          ? items.min_size_mb
          : DEFAULT_MIN_SIZE_MB;
      settingExtensions.value =
        items.allowed_extensions || DEFAULT_EXTENSIONS.join(", ");
    }
  );

  // Save settings on user input
  let saveTimer = null;
  function saveSettings() {
    const enabled = settingEnabled.checked;
    const minSizeMb = Math.max(0, parseFloat(settingMinSize.value) || 0);
    const allowedExtensions = settingExtensions.value;

    chrome.storage.local.set(
      {
        enabled,
        min_size_mb: minSizeMb,
        allowed_extensions: allowedExtensions
      },
      () => {
        if (chrome.runtime.lastError) return;
        savedNotice.classList.remove("hidden");
        if (saveTimer) clearTimeout(saveTimer);
        saveTimer = setTimeout(() => {
          savedNotice.classList.add("hidden");
          saveTimer = null;
        }, 1500);
      }
    );
  }

  settingEnabled.addEventListener("change", saveSettings);
  settingMinSize.addEventListener("input", saveSettings);
  settingExtensions.addEventListener("input", saveSettings);

  // Initial health check
  runHealthCheck(false);
});

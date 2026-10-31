/**
 * Browser extension detection, actions, and status mapping module.
 */

export const CHROME_WEB_STORE_URL = "";
export const EDGE_STORE_URL = "";
export const FIREFOX_AMO_URL = "";

export const DEFAULT_EXTENSION_PATH = "%LOCALAPPDATA%\\Sayl\\extension\\chromium";
export const DEFAULT_FIREFOX_EXTENSION_PATH = "%LOCALAPPDATA%\\Sayl\\extension\\firefox";

export const SUPPORTED_BROWSERS = [
  { id: "chrome", name: "Google Chrome", family: "chromium", defaultPageUrl: "chrome://extensions" },
  { id: "edge", name: "Microsoft Edge", family: "edge", defaultPageUrl: "edge://extensions" },
  { id: "brave", name: "Brave", family: "chromium", defaultPageUrl: "brave://extensions" },
  { id: "firefox", name: "Mozilla Firefox", family: "firefox", defaultPageUrl: "about:addons" },
  { id: "opera", name: "Opera", family: "chromium", defaultPageUrl: "opera://extensions" },
  { id: "vivaldi", name: "Vivaldi", family: "chromium", defaultPageUrl: "vivaldi://extensions" },
];

/**
 * Determines whether a browser action should open a store listing or the guided dialog.
 *
 * @param {string} browserId
 * @param {{ chrome?: string, edge?: string, firefox?: string }} storeUrls
 * @returns {{ type: 'store' | 'guided', url: string, available: boolean, reasonKey: string | null }}
 */
export function getBrowserAction(browserId, storeUrls = {}) {
  const id = (browserId || "").toLowerCase();
  const chromeStore = storeUrls.chrome !== undefined ? storeUrls.chrome : CHROME_WEB_STORE_URL;
  const edgeStore = storeUrls.edge !== undefined ? storeUrls.edge : EDGE_STORE_URL;
  const firefoxStore = storeUrls.firefox !== undefined ? storeUrls.firefox : FIREFOX_AMO_URL;

  if (id === "firefox") {
    return {
      type: "store",
      url: firefoxStore || "",
      available: Boolean(firefoxStore),
      reasonKey: firefoxStore ? null : "extension.storeUnavailable",
    };
  }

  if (id === "edge") {
    return {
      type: "store",
      url: edgeStore || "",
      available: Boolean(edgeStore),
      reasonKey: edgeStore ? null : "extension.storeUnavailable",
    };
  }

  // Chromium browsers: Chrome, Brave, Opera, Vivaldi
  if (chromeStore) {
    return {
      type: "store",
      url: chromeStore,
      available: true,
      reasonKey: null,
    };
  }

  return {
    type: "guided",
    url: "",
    available: true,
    reasonKey: null,
  };
}

/**
 * Maps per-browser status flags to localization keys and badge levels.
 *
 * Three states:
 * - connected (level: 'success')
 * - not installed (level: 'muted')
 * - not connected (level: 'warning')
 *
 * @param {{ installed?: boolean, connected?: boolean } | null | undefined} browser
 * @returns {{ key: 'connected' | 'not_installed' | 'not_connected', textKey: string, level: 'success' | 'warning' | 'muted' }}
 */
export function getBrowserStatusInfo(browser) {
  if (!browser || !browser.installed) {
    return {
      key: "not_installed",
      textKey: "extension.statusNotInstalled",
      level: "muted",
    };
  }

  if (browser.connected) {
    return {
      key: "connected",
      textKey: "extension.statusConnected",
      level: "success",
    };
  }

  return {
    key: "not_connected",
    textKey: "extension.statusNotInstalled",
    level: "warning",
  };
}

/**
 * Determines overall native-host bridge status across all detected browsers.
 *
 * Three states:
 * - connected (level: 'success'): at least one browser is connected
 * - not connected (level: 'warning'): at least one browser is installed but none is connected
 * - not installed (level: 'muted'): no supported browser is installed
 *
 * @param {object|null} _nativeStatus
 * @param {Array<object>} browserList
 * @returns {{ key: 'connected' | 'not_connected' | 'not_installed', textKey: string, level: 'success' | 'warning' | 'muted' }}
 */
export function getHostStatusSummary(_nativeStatus, browserList = []) {
  const list = Array.isArray(browserList) ? browserList : [];

  if (list.some((b) => b && b.connected)) {
    return {
      key: "connected",
      textKey: "extension.hostConnected",
      level: "success",
    };
  }

  if (list.some((b) => b && b.installed)) {
    return {
      key: "not_connected",
      textKey: "extension.hostNotInstalled",
      level: "warning",
    };
  }

  return {
    key: "not_installed",
    textKey: "extension.hostNotInstalled",
    level: "muted",
  };
}

/**
 * Checks whether any supported browser is detected/installed.
 *
 * @param {Array<object>} browserList
 * @returns {boolean}
 */
export function hasInstalledBrowsers(browserList) {
  if (!Array.isArray(browserList) || browserList.length === 0) {
    return false;
  }
  return browserList.some((b) => Boolean(b && b.installed));
}

/**
 * SVG icons for each supported browser.
 */
export const BROWSER_ICONS = {
  chrome: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="4"/><line x1="21.17" y1="8" x2="12" y2="8"/><line x1="3.95" y1="6.06" x2="8.54" y2="14"/><line x1="10.88" y1="21.94" x2="15.46" y2="14"/></svg>`,
  edge: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18.8 8.6c-.3-2.6-2.5-4.6-5.2-4.6-3 0-5.5 2.5-5.5 5.5 0 .6.1 1.2.3 1.8-.8-.4-1.7-.6-2.7-.6-3 0-5.4 2.4-5.4 5.4 0 2.8 2.1 5.1 4.8 5.4.6 0 1.2-.1 1.8-.3 1.2 1.5 3 2.5 5.1 2.5 3.5 0 6.4-2.7 6.6-6.2.8.3 1.7.5 2.6.5 3.2 0 5.8-2.6 5.8-5.8 0-1.8-.8-3.4-2.1-4.5-.4.5-.9.9-1.4 1.2z"/></svg>`,
  brave: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2L4 5v6.5C4 16.5 7.4 21 12 22c4.6-1 8-5.5 8-10.5V5l-8-3z"/><path d="M9 12l2 2 4-4"/></svg>`,
  firefox: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 2a10 10 0 0 1 8.66 15A10 10 0 0 0 12 7a5 5 0 0 0-5 5c0 3 2 5 5 5"/></svg>`,
  opera: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><ellipse cx="12" cy="12" rx="4" ry="7"/></svg>`,
  vivaldi: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M8 9l4 7 4-7"/></svg>`,
};

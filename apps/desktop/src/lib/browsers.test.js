import { describe, it, expect } from "vitest";
import {
  CHROME_WEB_STORE_URL,
  EDGE_STORE_URL,
  FIREFOX_AMO_URL,
  SUPPORTED_BROWSERS,
  getBrowserAction,
  getBrowserStatusInfo,
  getHostStatusSummary,
  hasInstalledBrowsers,
} from "./browsers.js";

describe("browsers.js pure logic", () => {
  it("initial URL constants default to empty strings", () => {
    expect(CHROME_WEB_STORE_URL).toBe("");
    expect(EDGE_STORE_URL).toBe("");
    expect(FIREFOX_AMO_URL).toBe("");
  });

  it("lists all 6 supported browsers", () => {
    expect(SUPPORTED_BROWSERS).toHaveLength(6);
    const ids = SUPPORTED_BROWSERS.map((b) => b.id);
    expect(ids).toContain("chrome");
    expect(ids).toContain("edge");
    expect(ids).toContain("brave");
    expect(ids).toContain("firefox");
    expect(ids).toContain("opera");
    expect(ids).toContain("vivaldi");
  });

  it("decides store vs guided for Edge and Firefox based on URL constants", () => {
    // When constants are empty (default)
    const edgeDefault = getBrowserAction("edge");
    expect(edgeDefault.type).toBe("store");
    expect(edgeDefault.available).toBe(false);
    expect(edgeDefault.reasonKey).toBe("extension.storeUnavailable");

    const ffDefault = getBrowserAction("firefox");
    expect(ffDefault.type).toBe("store");
    expect(ffDefault.available).toBe(false);
    expect(ffDefault.reasonKey).toBe("extension.storeUnavailable");

    // When store URLs are provided
    const edgeWithUrl = getBrowserAction("edge", { edge: "https://microsoftedge.microsoft.com/addons/detail/123" });
    expect(edgeWithUrl.type).toBe("store");
    expect(edgeWithUrl.available).toBe(true);
    expect(edgeWithUrl.url).toBe("https://microsoftedge.microsoft.com/addons/detail/123");
    expect(edgeWithUrl.reasonKey).toBe(null);

    const ffWithUrl = getBrowserAction("firefox", { firefox: "https://addons.mozilla.org/firefox/addon/sayl" });
    expect(ffWithUrl.type).toBe("store");
    expect(ffWithUrl.available).toBe(true);
    expect(ffWithUrl.url).toBe("https://addons.mozilla.org/firefox/addon/sayl");
  });

  it("decides guided dialog for Chromium browsers when store URL is empty, switches to store if present", () => {
    const chromeDefault = getBrowserAction("chrome");
    expect(chromeDefault.type).toBe("guided");
    expect(chromeDefault.available).toBe(true);

    const braveDefault = getBrowserAction("brave");
    expect(braveDefault.type).toBe("guided");
    expect(braveDefault.available).toBe(true);

    const operaDefault = getBrowserAction("opera");
    expect(operaDefault.type).toBe("guided");

    const vivaldiDefault = getBrowserAction("vivaldi");
    expect(vivaldiDefault.type).toBe("guided");

    // If CHROME_WEB_STORE_URL becomes populated
    const chromeWithStore = getBrowserAction("chrome", {
      chrome: "https://chromewebstore.google.com/detail/sayl/abc",
    });
    expect(chromeWithStore.type).toBe("store");
    expect(chromeWithStore.available).toBe(true);
    expect(chromeWithStore.url).toBe("https://chromewebstore.google.com/detail/sayl/abc");

    const braveWithStore = getBrowserAction("brave", {
      chrome: "https://chromewebstore.google.com/detail/sayl/abc",
    });
    expect(braveWithStore.type).toBe("store");
    expect(braveWithStore.available).toBe(true);
  });

  it("maps per-browser status flags to localized text keys", () => {
    expect(getBrowserStatusInfo(null).key).toBe("not_installed");
    expect(getBrowserStatusInfo({ installed: false }).key).toBe("not_installed");
    expect(getBrowserStatusInfo({ installed: false }).textKey).toBe("extension.statusNotInstalled");

    expect(getBrowserStatusInfo({ installed: true, connected: true }).key).toBe("connected");
    expect(getBrowserStatusInfo({ installed: true, connected: true }).textKey).toBe("extension.statusConnected");

    expect(getBrowserStatusInfo({ installed: true, host_registered: true, connected: false }).key).toBe("registered");
    expect(getBrowserStatusInfo({ installed: true, host_registered: true, connected: false }).textKey).toBe(
      "extension.statusRegistered"
    );

    expect(getBrowserStatusInfo({ installed: true, host_registered: false, connected: false }).key).toBe("unregistered");
  });

  it("determines overall host summary status correctly", () => {
    // When at least one browser is connected
    const connectedList = [
      { id: "chrome", installed: true, host_registered: true, connected: true },
      { id: "edge", installed: true, host_registered: false, connected: false },
    ];
    expect(getHostStatusSummary(null, connectedList).key).toBe("connected");
    expect(getHostStatusSummary(null, connectedList).textKey).toBe("extension.hostConnected");

    // When registered but not connected
    const registeredList = [
      { id: "chrome", installed: true, host_registered: true, connected: false },
    ];
    expect(getHostStatusSummary(null, registeredList).key).toBe("registered");
    expect(getHostStatusSummary(null, registeredList).textKey).toBe("extension.hostRegistered");

    // When nativeStatus has registered flag
    const nativeStatus = { registered: { chrome: true, firefox: false } };
    expect(getHostStatusSummary(nativeStatus, []).key).toBe("registered");

    // When nothing is registered or connected
    expect(getHostStatusSummary(null, []).key).toBe("not_installed");
    expect(getHostStatusSummary(null, []).textKey).toBe("extension.hostNotInstalled");
  });

  it("detects whether any installed browser exists (empty state check)", () => {
    expect(hasInstalledBrowsers([])).toBe(false);
    expect(hasInstalledBrowsers(null)).toBe(false);
    expect(hasInstalledBrowsers([{ id: "chrome", installed: false }])).toBe(false);
    expect(
      hasInstalledBrowsers([
        { id: "chrome", installed: false },
        { id: "firefox", installed: true },
      ])
    ).toBe(true);
  });
});

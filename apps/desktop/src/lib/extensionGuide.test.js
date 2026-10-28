import { describe, it, expect } from "vitest";
import {
  EXTENSION_GUIDE_TOTAL_STEPS,
  EXTENSION_SEEN_FRESH_MS,
  buildStepCounterLabel,
  mapPingResultToStatus,
  extensionSeenStatus,
  resolveExtensionGuideDisplay,
} from "./extensionGuide.js";

describe("extensionGuide.js", () => {
  const arStepTemplate = "خطوة {current} من {total}";

  it("builds step counter from template without a glued prefix", () => {
    expect(buildStepCounterLabel(1, EXTENSION_GUIDE_TOTAL_STEPS, arStepTemplate)).toBe(
      "خطوة 1 من 2"
    );
    expect(buildStepCounterLabel(2, EXTENSION_GUIDE_TOTAL_STEPS, arStepTemplate)).toBe(
      "خطوة 2 من 2"
    );
    expect(buildStepCounterLabel(1, 2, arStepTemplate)).not.toMatch(/^2\s/);
  });

  it("maps successful host ping to hostReady, never statusConnected", () => {
    const status = mapPingResultToStatus({ ok: true, message: "OK", roundtrip_ms: 4 });
    expect(status.variant).toBe("ready");
    expect(status.statusKey).toBe("extension.hostReady");
    expect(status.statusKey).not.toBe("extension.statusConnected");
  });

  it("maps failed ping message to error display without connected status", () => {
    const status = mapPingResultToStatus({ ok: false, message: "Extension not responding" });
    expect(status.variant).toBe("error");
    expect(status.statusKey).toBeNull();
    expect(status.message).toBe("Extension not responding");
  });

  it("does not treat browser.connected as connected when ping is not ok", () => {
    const status = mapPingResultToStatus({ ok: false, message: "Host only" }, true);
    expect(status.variant).not.toBe("success");
    expect(status.statusKey).not.toBe("extension.statusConnected");
  });

  it("maps unresolved failed ping without message to pending without returnInstruction", () => {
    const status = mapPingResultToStatus({ ok: false, message: "" });
    expect(status.variant).toBe("pending");
    expect(status.statusKey).toBeNull();
  });

  it("uses checking key while ping is unresolved", () => {
    const status = mapPingResultToStatus(null);
    expect(status.variant).toBe("checking");
    expect(status.statusKey).toBe("extension.statusChecking");
  });

  it("extensionSeenStatus returns connected only for fresh finite age", () => {
    expect(extensionSeenStatus(0).statusKey).toBe("extension.statusConnected");
    expect(extensionSeenStatus(EXTENSION_SEEN_FRESH_MS - 1).statusKey).toBe(
      "extension.statusConnected"
    );
    expect(extensionSeenStatus(EXTENSION_SEEN_FRESH_MS).statusKey).toBe("extension.hostReady");
    expect(extensionSeenStatus(null).statusKey).toBe("extension.hostReady");
    expect(extensionSeenStatus(Number.NaN).statusKey).toBe("extension.hostReady");
  });

  it("resolveExtensionGuideDisplay shows connected only via extensionSeenStatus", () => {
    const hostReady = mapPingResultToStatus({ ok: true, message: "OK" });
    expect(
      resolveExtensionGuideDisplay(hostReady, EXTENSION_SEEN_FRESH_MS).statusKey
    ).toBe("extension.hostReady");
    expect(resolveExtensionGuideDisplay(hostReady, 100).statusKey).toBe(
      "extension.statusConnected"
    );
    expect(resolveExtensionGuideDisplay(hostReady, 100).variant).toBe("success");
  });
});

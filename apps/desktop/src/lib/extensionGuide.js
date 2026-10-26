/** @typedef {{ ok?: boolean, message?: string, roundtrip_ms?: number }} NativeHostPing */

export const EXTENSION_GUIDE_TOTAL_STEPS = 2;

/** Extension health marker is fresh when younger than this many milliseconds. */
export const EXTENSION_SEEN_FRESH_MS = 30_000;

/**
 * Builds the step counter label from template placeholders (no glued numeric prefix).
 * @param {number} current
 * @param {number} total
 * @param {string} template
 */
export function buildStepCounterLabel(current, total, template) {
  const cur = Number.isFinite(current) ? Math.max(1, Math.floor(current)) : 1;
  const tot = Number.isFinite(total) ? Math.max(1, Math.floor(total)) : 1;
  return String(template)
    .replace("{current}", String(cur))
    .replace("{total}", String(tot));
}

/**
 * Maps `test_native_host` ping to dialog status. Never returns `extension.statusConnected`.
 * @param {NativeHostPing | null | undefined} ping
 * @param {boolean} [browserConnected]
 * @returns {{ variant: "checking" | "ready" | "pending" | "error", statusKey: string | null, message: string | null }}
 */
export function mapPingResultToStatus(ping, browserConnected = false) {
  void browserConnected;

  if (ping == null) {
    return {
      variant: "checking",
      statusKey: "extension.statusChecking",
      message: null,
    };
  }

  if (ping.ok === true) {
    return {
      variant: "ready",
      statusKey: "extension.hostReady",
      message: null,
    };
  }

  const message =
    typeof ping.message === "string" && ping.message.trim() ? ping.message.trim() : null;

  if (message) {
    return {
      variant: "error",
      statusKey: null,
      message,
    };
  }

  return {
    variant: "pending",
    statusKey: "extension.returnInstruction",
    message: null,
  };
}

/**
 * Maps extension health marker age to connected vs host-only status.
 * @param {number | null | undefined} ageMs
 * @returns {{ statusKey: "extension.statusConnected" | "extension.hostReady" }}
 */
export function extensionSeenStatus(ageMs) {
  if (typeof ageMs === "number" && Number.isFinite(ageMs) && ageMs < EXTENSION_SEEN_FRESH_MS) {
    return { statusKey: "extension.statusConnected" };
  }
  return { statusKey: "extension.hostReady" };
}

/**
 * Combines host ping status with extension-seen age for the guide dialog.
 * @param {{ variant: string, statusKey: string | null, message: string | null }} hostStatus
 * @param {number | null | undefined} ageMs
 */
export function resolveExtensionGuideDisplay(hostStatus, ageMs) {
  if (hostStatus.variant !== "ready") {
    return hostStatus;
  }
  const seen = extensionSeenStatus(ageMs);
  if (seen.statusKey === "extension.statusConnected") {
    return { variant: "success", statusKey: seen.statusKey, message: null };
  }
  return { variant: "ready", statusKey: seen.statusKey, message: null };
}

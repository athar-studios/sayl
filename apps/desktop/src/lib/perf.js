/**
 * Lightweight frontend performance instrumentation module.
 * Measures first paint, IPC invocations, and click action settling times.
 * Reports to backend command `report_frontend_perf` throttled at most once every 5 seconds.
 */

import { safeInvoke } from "./tauri.js";

// Pure calculation helpers (unit-testable)
export function calculateMedian(numbers) {
  if (!Array.isArray(numbers) || numbers.length === 0) return 0;
  const sorted = [...numbers].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  const val = sorted.length % 2 !== 0 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
  return Number(val.toFixed(2));
}

export function calculateMax(numbers) {
  if (!Array.isArray(numbers) || numbers.length === 0) return 0;
  return Number(Math.max(...numbers).toFixed(2));
}

export function calculateRate(count, durationMs) {
  if (!count || count <= 0 || !durationMs || durationMs <= 0) return 0;
  const durationSec = durationMs / 1000;
  return Number((count / durationSec).toFixed(2));
}

export function shouldThrottle(lastReportTime, currentTime, minIntervalMs = 5000) {
  if (!lastReportTime) return false;
  return currentTime - lastReportTime < minIntervalMs;
}

// Runtime state
let initTime = typeof performance !== "undefined" ? performance.now() : Date.now();
let ipcTotal = 0;
let firstPaintMs = null;
const clickSamples = [];
const MAX_CLICK_SAMPLES = 20;
let lastReportTime = 0;

export function resetPerf() {
  initTime = typeof performance !== "undefined" ? performance.now() : Date.now();
  ipcTotal = 0;
  firstPaintMs = null;
  clickSamples.length = 0;
  lastReportTime = 0;
}

export function getClickStats() {
  return {
    median: calculateMedian(clickSamples),
    max: calculateMax(clickSamples),
    count: clickSamples.length,
    samples: [...clickSamples],
  };
}

export function addClickSample(durationMs) {
  clickSamples.push(Number(durationMs.toFixed(2)));
  if (clickSamples.length > MAX_CLICK_SAMPLES) {
    clickSamples.shift();
  }
}

/**
 * Record an invoke call before executing safeInvoke
 */
export function recordInvoke() {
  ipcTotal += 1;
  triggerReport(false);
}

/**
 * Begin measuring a click action
 * @returns {number} Start timestamp from performance.now()
 */
export function startClick() {
  return typeof performance !== "undefined" ? performance.now() : Date.now();
}

/**
 * Finish measuring a click action, scheduling duration calculation
 * after two nested requestAnimationFrames.
 *
 * @param {number} startTime Start timestamp from startClick()
 * @param {string} [_kind] Optional label for the action
 */
export function endClick(startTime, _kind) {
  const start = startTime || (typeof performance !== "undefined" ? performance.now() : Date.now());

  if (typeof requestAnimationFrame === "function") {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        const now = typeof performance !== "undefined" ? performance.now() : Date.now();
        const duration = Math.max(0, now - start);
        addClickSample(duration);
        triggerReport(false);
      });
    });
  } else {
    // Non-browser / test environment fallback
    const now = typeof performance !== "undefined" ? performance.now() : Date.now();
    const duration = Math.max(0, now - start);
    addClickSample(duration);
    triggerReport(false);
  }
}

/**
 * Record first paint timestamp and trigger immediate report
 * @param {number} ms
 */
export function recordFirstPaint(ms) {
  firstPaintMs = Number(Number(ms).toFixed(2));
  triggerReport(true);
}

/**
 * Throttled reporter invoking report_frontend_perf
 * @param {boolean} [force=false]
 */
export function triggerReport(force = false) {
  const now = typeof performance !== "undefined" ? performance.now() : Date.now();

  if (!force && shouldThrottle(lastReportTime, now, 5000)) {
    return;
  }

  lastReportTime = now;

  const elapsedMs = Math.max(1, now - initTime);
  const clickMs = calculateMedian(clickSamples);
  const ipcPerSec = calculateRate(ipcTotal, elapsedMs);

  const payload = {
    firstPaintMs,
    clickMs,
    ipcTotal,
    ipcPerSec,
  };

  try {
    safeInvoke("report_frontend_perf", { payload }).catch(() => {
      // Swallowed silently when backend doesn't implement or SAYL_PERF != 1
    });
  } catch {
    // Swallowed silently
  }
}

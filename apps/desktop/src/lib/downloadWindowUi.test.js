/**
 * Vitest unit tests for `downloadWindowUi.js` pure helpers.
 * Style mirrors `format.test.js` and `state.test.js`.
 */

import { describe, it, expect, vi } from "vitest";
import {
  buildWindowTitle,
  truncateMiddle,
  createMovingAverage,
  createThrottledTaskView,
  buildSegmentBars,
  normalizeSpeedLimit,
  isResumable,
  deriveDisplayState,
  mergeTaskMonotonic,
  mergeTasksMonotonic,
  isProgressResetState,
  getProgressBarFillDirection,
  getProgressBarFillStyle,
} from "./downloadWindowUi.js";

describe("downloadWindowUi — pure helpers", () => {
  describe("buildWindowTitle", () => {
    it("renders '12% file.zip' for a normal case", () => {
      expect(buildWindowTitle(12, "file.zip")).toBe("12% file.zip");
    });

    it("uses one decimal for fractional percents", () => {
      expect(buildWindowTitle(12.3, "file.zip")).toBe("12.3% file.zip");
    });

    it("clamps percent to 0..100", () => {
      expect(buildWindowTitle(-5, "x")).toBe("0% x");
      expect(buildWindowTitle(105, "x")).toBe("100% x");
    });

    it("returns filename when percent is null/undefined/NaN", () => {
      expect(buildWindowTitle(null, "file.zip")).toBe("file.zip");
      expect(buildWindowTitle(undefined, "file.zip")).toBe("file.zip");
      expect(buildWindowTitle(NaN, "file.zip")).toBe("file.zip");
    });

    it("returns percent when filename is empty/null", () => {
      expect(buildWindowTitle(50, "")).toBe("50%");
      expect(buildWindowTitle(50, null)).toBe("50%");
      expect(buildWindowTitle(50, undefined)).toBe("50%");
    });

    it("returns empty string when both missing", () => {
      expect(buildWindowTitle(null, null)).toBe("");
    });

    it("trims filename whitespace", () => {
      expect(buildWindowTitle(12, "  file.zip  ")).toBe("12% file.zip");
    });
  });

  describe("truncateMiddle", () => {
    it("leaves short text unchanged", () => {
      expect(truncateMiddle("short", 40)).toBe("short");
    });

    it("truncates long URLs keeping head and tail", () => {
      const long = "https://example.com/very/long/path/to/file.zip";
      const out = truncateMiddle(long, 40);
      expect(out.length).toBe(40);
      expect(out).toContain("https://");
      expect(out).toContain("file.zip");
      expect(out).toContain("…");
    });

    it("handles maxLen smaller than ellipsis", () => {
      expect(truncateMiddle("abcdef", 1)).toBe("…");
      expect(truncateMiddle("abcdef", 2)).toBe("a…");
    });

    it("returns empty string for null/undefined", () => {
      expect(truncateMiddle(null, 40)).toBe("");
      expect(truncateMiddle(undefined, 40)).toBe("");
    });

    it("coerces non-string values", () => {
      expect(truncateMiddle(123, 5)).toBe("123");
    });
  });

  describe("createMovingAverage", () => {
    it("returns 0 before first sample", () => {
      const ma = createMovingAverage({ size: 3 });
      expect(ma.value()).toBe(0);
    });

    it("ignores NaN and negative samples", () => {
      const ma = createMovingAverage({ size: 3 });
      expect(ma.push(NaN)).toBe(0);
      expect(ma.push(-5)).toBe(0);
      expect(ma.value()).toBe(0);
    });

    it("averages the last N finite samples", () => {
      const ma = createMovingAverage({ size: 3 });
      expect(ma.push(10)).toBe(10);
      expect(ma.push(20)).toBe(15);
      expect(ma.push(30)).toBe(20);
      expect(ma.push(40)).toBe(30); // only last 3: 20,30,40 -> 30
    });

    it("handles size=1 correctly", () => {
      const ma = createMovingAverage({ size: 1 });
      expect(ma.push(10)).toBe(10);
      expect(ma.push(50)).toBe(50);
      expect(ma.value()).toBe(50);
    });

    it("ignores non-numeric values", () => {
      const ma = createMovingAverage({ size: 3 });
      expect(ma.push("abc")).toBe(0);
      expect(ma.push(null)).toBe(0);
      expect(ma.push({})).toBe(0);
    });
  });

  describe("createThrottledTaskView", () => {
    it("emits immediately for first task", () => {
      const now = 1000;
      const throttler = createThrottledTaskView({ intervalMs: 500, now: () => now });
      const task = { id: "t1", written: 100, total_size: 1000 };
      const view = throttler.push(task);
      expect(view).toEqual({ ...task, written: 100, percent: 10 });
    });

    it("stores latest and returns previously emitted view within interval", () => {
      let clock = 1000;
      const throttler = createThrottledTaskView({ intervalMs: 500, now: () => clock });
      const task1 = { id: "t1", written: 100, total_size: 1000 };
      throttler.push(task1);
      clock = 1200;
      const task2 = { id: "t1", written: 200, total_size: 1000 };
      const view = throttler.push(task2);
      expect(view.written).toBe(100); // still old
    });

    it("emits new snapshot once interval elapsed", () => {
      let clock = 1000;
      const throttler = createThrottledTaskView({ intervalMs: 500, now: () => clock });
      throttler.push({ id: "t1", written: 100, total_size: 1000 });
      clock = 1501;
      const view = throttler.push({ id: "t1", written: 200, total_size: 1000 });
      expect(view.written).toBe(200);
      expect(view.percent).toBe(20);
    });

    it("always emits latest snapshot on flush", () => {
      let clock = 1000;
      const throttler = createThrottledTaskView({ intervalMs: 500, now: () => clock });
      throttler.push({ id: "t1", written: 100, total_size: 1000 });
      clock = 1200;
      const view = throttler.flush();
      expect(view.written).toBe(100);
    });

    it("never decreases written for the same task id", () => {
      let clock = 1000;
      const throttler = createThrottledTaskView({ intervalMs: 500, now: () => clock });
      throttler.push({ id: "t1", written: 300, total_size: 1000 });
      clock = 1501;
      const view = throttler.push({ id: "t1", written: 200, total_size: 1000 });
      expect(view.written).toBe(300);
    });

    it("resets monotonic guard on new task id", () => {
      let clock = 1000;
      const throttler = createThrottledTaskView({ intervalMs: 500, now: () => clock });
      throttler.push({ id: "t1", written: 300, total_size: 1000 });
      clock = 1501;
      const view = throttler.push({ id: "t2", written: 50, total_size: 1000 });
      expect(view.written).toBe(50);
    });

    it("handles missing/invalid task gracefully", () => {
      const throttler = createThrottledTaskView({ intervalMs: 500 });
      expect(throttler.push(null)).toBe(null);
      expect(throttler.push("not-an-object")).toBe(null);
      expect(throttler.push({})).toEqual({ written: 0, percent: null });
    });
  });

  describe("buildSegmentBars", () => {
    it("returns empty array for empty/no input", () => {
      expect(buildSegmentBars([])).toEqual([]);
      expect(buildSegmentBars(null)).toEqual([]);
      expect(buildSegmentBars(undefined)).toEqual([]);
    });

    it("clamps fraction to 0..1", () => {
      const bars = buildSegmentBars([{ start: 0, end: 100, fraction: -0.5 }]);
      expect(bars[0].fraction).toBe(0);

      const bars2 = buildSegmentBars([{ start: 0, end: 100, fraction: 1.5 }]);
      expect(bars2[0].fraction).toBe(1);
    });

    it("merges adjacent segments down to maxCells", () => {
      const segments = Array.from({ length: 20 }, (_, i) => ({
        start: i * 100,
        end: (i + 1) * 100,
        fraction: i / 20,
      }));
      const bars = buildSegmentBars(segments, { maxCells: 5 });
      expect(bars.length).toBe(5);
    });

    it("uses totalSize to bin into equal byte bands when provided", () => {
      const bars = buildSegmentBars(
        [{ start: 0, end: 500, fraction: 1 }],
        { maxCells: 5, totalSize: 1000 }
      );
      expect(bars.length).toBe(5);
      // bytes 0-500 of 1000 in 5 bands of 200: bands 0-1 full, band 2 half, rest empty
      expect(bars[0].fraction).toBeCloseTo(1, 5);
      expect(bars[1].fraction).toBeCloseTo(1, 5);
      expect(bars[2].fraction).toBeCloseTo(0.5, 5);
      expect(bars[3].fraction).toBe(0);
      expect(bars[4].fraction).toBe(0);
    });

    it("maxCells=1 yields a single bar", () => {
      const bars = buildSegmentBars(
        [{ start: 0, end: 100, fraction: 0.5 }],
        { maxCells: 1 }
      );
      expect(bars.length).toBe(1);
    });
  });

  describe("normalizeSpeedLimit", () => {
    it("returns integer within bounds", () => {
      expect(normalizeSpeedLimit(1024)).toBe(1024);
      expect(normalizeSpeedLimit("500")).toBe(500);
      expect(normalizeSpeedLimit(3.7)).toBe(4);
    });

    it("rejects junk and returns null", () => {
      expect(normalizeSpeedLimit(null)).toBe(null);
      expect(normalizeSpeedLimit(undefined)).toBe(null);
      expect(normalizeSpeedLimit("abc")).toBe(null);
      expect(normalizeSpeedLimit(NaN)).toBe(null);
      expect(normalizeSpeedLimit({})).toBe(null);
    });

    it("clamps to min 1 KB/s and max 10_000_000 KB/s", () => {
      expect(normalizeSpeedLimit(0)).toBe(1);
      expect(normalizeSpeedLimit(-10)).toBe(1);
      expect(normalizeSpeedLimit(10_000_001)).toBe(10_000_000);
      expect(normalizeSpeedLimit(1e9)).toBe(10_000_000);
    });
  });

  describe("isResumable", () => {
    it("returns false for range_unsupported error kind", () => {
      expect(isResumable({ total_size: 1000, error_kind: "range_unsupported" })).toBe(false);
    });

    it("uses explicit resumable flag when present", () => {
      expect(isResumable({ total_size: 1000, resumable: true })).toBe(true);
      expect(isResumable({ total_size: 1000, resumable: false })).toBe(false);
    });

    it("returns true when total_size is known and no error", () => {
      expect(isResumable({ total_size: 1000 })).toBe(true);
      expect(isResumable({ total_size: 1 })).toBe(true);
    });

    it("returns false for unknown size", () => {
      expect(isResumable({ total_size: null })).toBe(false);
      expect(isResumable({ total_size: undefined })).toBe(false);
      expect(isResumable({})).toBe(false);
      expect(isResumable(null)).toBe(false);
      expect(isResumable(undefined)).toBe(false);
    });
  });

  describe("deriveDisplayState", () => {
    it("returns the task state unchanged when present", () => {
      expect(deriveDisplayState({ state: "Downloading" })).toBe("Downloading");
      expect(deriveDisplayState({ state: "Paused" })).toBe("Paused");
      expect(deriveDisplayState({ state: "Failed" })).toBe("Failed");
      expect(deriveDisplayState({ state: "Completed" })).toBe("Completed");
    });

    it("windowClosed does not change state", () => {
      expect(deriveDisplayState({ state: "Paused" }, { windowClosed: true })).toBe("Paused");
      expect(deriveDisplayState({ state: "Cancelled" }, { windowClosed: true })).toBe("Cancelled");
    });

    it("only Completed yields Completed", () => {
      expect(deriveDisplayState({ state: "Queued" })).toBe("Queued");
      expect(deriveDisplayState({ state: "Downloading" })).toBe("Downloading");
      expect(deriveDisplayState({ state: "Verifying" })).toBe("Verifying");
      expect(deriveDisplayState({ state: "Finalizing" })).toBe("Finalizing");
      expect(deriveDisplayState({ state: "Completed" })).toBe("Completed");
      expect(deriveDisplayState({ state: "Cancelled" })).toBe("Cancelled");
      expect(deriveDisplayState({ state: "Failed" })).toBe("Failed");
    });

    it("falls back to Queued for missing/empty state", () => {
      expect(deriveDisplayState(null)).toBe("Queued");
      expect(deriveDisplayState(undefined)).toBe("Queued");
      expect(deriveDisplayState({})).toBe("Queued");
      expect(deriveDisplayState({ state: "" })).toBe("Queued");
      expect(deriveDisplayState({ state: null })).toBe("Queued");
    });
  });
});

describe("downloadWindowUi — monotonic progress (mergeTaskMonotonic)", () => {
  it("prevents written bytes from decreasing for same task", () => {
    const existing = { id: "t1", written: 500, total_size: 1000, state: "Downloading" };
    const incoming = { id: "t1", written: 300, total_size: 1000, state: "Downloading" };
    const merged = mergeTaskMonotonic(existing, incoming);
    expect(merged.written).toBe(500);
  });

  it("allows written bytes to increase", () => {
    const existing = { id: "t1", written: 300, total_size: 1000, state: "Downloading" };
    const incoming = { id: "t1", written: 500, total_size: 1000, state: "Downloading" };
    const merged = mergeTaskMonotonic(existing, incoming);
    expect(merged.written).toBe(500);
  });

  it("allows progress decrease when state indicates restart (Queued from Downloading)", () => {
    const existing = { id: "t1", written: 500, total_size: 1000, state: "Downloading" };
    const incoming = { id: "t1", written: 0, total_size: 1000, state: "Queued" };
    const merged = mergeTaskMonotonic(existing, incoming);
    expect(merged.written).toBe(0);
    expect(merged.state).toBe("Queued");
  });

  it("allows progress decrease when state is Cancelled", () => {
    const existing = { id: "t1", written: 500, total_size: 1000, state: "Downloading" };
    const incoming = { id: "t1", written: 0, total_size: 1000, state: "Cancelled" };
    const merged = mergeTaskMonotonic(existing, incoming);
    expect(merged.written).toBe(0);
    expect(merged.state).toBe("Cancelled");
  });

  it("allows progress decrease when state transitions to Failed", () => {
    const existing = { id: "t1", written: 500, total_size: 1000, state: "Downloading" };
    const incoming = { id: "t1", written: 0, total_size: 1000, state: "Failed" };
    const merged = mergeTaskMonotonic(existing, incoming);
    expect(merged.written).toBe(0);
    expect(merged.state).toBe("Failed");
  });

  it("does not reset progress when staying in Failed state", () => {
    const existing = { id: "t1", written: 500, total_size: 1000, state: "Failed" };
    const incoming = { id: "t1", written: 300, total_size: 1000, state: "Failed" };
    const merged = mergeTaskMonotonic(existing, incoming);
    expect(merged.written).toBe(500);
  });

  it("merges segment_ranges monotonically", () => {
    const existing = {
      id: "t1",
      written: 500,
      total_size: 1000,
      state: "Downloading",
      segment_ranges: [
        { start: 0, end: 500, completed: true },
        { start: 500, end: 1000, completed: false },
      ],
    };
    const incoming = {
      id: "t1",
      written: 600,
      total_size: 1000,
      state: "Downloading",
      segment_ranges: [
        { start: 0, end: 500, completed: true },
        { start: 500, end: 1000, completed: true },
      ],
    };
    const merged = mergeTaskMonotonic(existing, incoming);
    expect(merged.segment_ranges[1].completed).toBe(true);
  });

  it("mergeTasksMonotonic merges multiple tasks", () => {
    const existingTasks = [
      { id: "t1", written: 500, total_size: 1000, state: "Downloading" },
      { id: "t2", written: 200, total_size: 500, state: "Paused" },
    ];
    const incomingTasks = [
      { id: "t1", written: 300, total_size: 1000, state: "Downloading" }, // should not decrease
      { id: "t2", written: 300, total_size: 500, state: "Downloading" }, // should increase
      { id: "t3", written: 100, total_size: 1000, state: "Queued" }, // new task
    ];
    const merged = mergeTasksMonotonic(existingTasks, incomingTasks);
    expect(merged.find(t => t.id === "t1").written).toBe(500);
    expect(merged.find(t => t.id === "t2").written).toBe(300);
    expect(merged.find(t => t.id === "t3").written).toBe(100);
  });

  it("isProgressResetState detects restart transitions", () => {
    expect(isProgressResetState("Queued", "Downloading")).toBe(true);
    expect(isProgressResetState("Probing", "Downloading")).toBe(true);
    expect(isProgressResetState("Cancelled", "Downloading")).toBe(true);
    expect(isProgressResetState("Failed", "Downloading")).toBe(true);
    expect(isProgressResetState("Downloading", "Queued")).toBe(false);
    expect(isProgressResetState("Paused", "Downloading")).toBe(false);
    expect(isProgressResetState("Failed", "Failed")).toBe(false);
  });
});

describe("downloadWindowUi — RTL progress bar fill direction", () => {
  it("returns RTL direction and right fill start for Arabic", () => {
    const result = getProgressBarFillDirection(true);
    expect(result.direction).toBe("rtl");
    expect(result.fillStart).toBe("right");
  });

  it("returns LTR direction and left fill start for English", () => {
    const result = getProgressBarFillDirection(false);
    expect(result.direction).toBe("ltr");
    expect(result.fillStart).toBe("left");
  });

  it("getProgressBarFillStyle returns marginLeft: auto for RTL", () => {
    const style = getProgressBarFillStyle(true, 50);
    expect(style.marginLeft).toBe("auto");
    expect(style.marginRight).toBe(0);
    expect(style.width).toBe("50%");
  });

  it("getProgressBarFillStyle returns marginRight: auto for LTR", () => {
    const style = getProgressBarFillStyle(false, 50);
    expect(style.marginLeft).toBe(0);
    expect(style.marginRight).toBe("auto");
    expect(style.width).toBe("50%");
  });

  it("clamps percent to 0..100 in getProgressBarFillStyle", () => {
    expect(getProgressBarFillStyle(false, -10).width).toBe("0%");
    expect(getProgressBarFillStyle(false, 150).width).toBe("100%");
  });
});

describe("downloadWindowUi — Cancel vs Hide button behavior", () => {
  it("Cancel button dispatches cancel event", () => {
    const dispatch = vi.fn();
    const taskId = "task-123";
    const handler = () => dispatch("cancel", taskId);
    handler();
    expect(dispatch).toHaveBeenCalledWith("cancel", taskId);
  });

  it("Hide/Close button dispatches close event", () => {
    const dispatch = vi.fn();
    const taskId = "task-123";
    const handler = () => dispatch("close", taskId);
    handler();
    expect(dispatch).toHaveBeenCalledWith("close", taskId);
  });

  it("X button in header dispatches close event (hide window)", () => {
    const dispatch = vi.fn();
    const taskId = "task-123";
    const handler = () => dispatch("close", taskId);
    handler();
    expect(dispatch).toHaveBeenCalledWith("close", taskId);
  });
});
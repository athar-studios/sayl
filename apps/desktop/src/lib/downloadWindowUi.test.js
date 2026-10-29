/**
 * Vitest unit tests for `downloadWindowUi.js` pure helpers.
 * Style mirrors `format.test.js` and `state.test.js`.
 */

import { describe, it, expect } from "vitest";
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
  progressPercent,
  buildStatusRows,
  buildConnectionSegments,
  buildConnectionTableRows,
  formatRangeInfo,
  buildSpeedLimitState,
  getWindowActionFlags,
  buildCompleteDetails,
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
  it("offers cancel while downloading and never swaps it for hide", () => {
    const flags = getWindowActionFlags({ state: "Downloading" });
    expect(flags.cancel).toBe(true);
    expect(flags.hide).toBe(true);
    expect(flags.pause).toBe(true);
  });

  it("hides the window without cancelling when the task is completed", () => {
    const flags = getWindowActionFlags({ state: "Completed" });
    expect(flags.cancel).toBe(false);
    expect(flags.hide).toBe(true);
    expect(flags.openFile).toBe(true);
    expect(flags.reveal).toBe(true);
  });
});

describe("downloadWindowUi — progressPercent", () => {
  it("returns a 0..100 percentage with one decimal", () => {
    expect(progressPercent(500, 1000)).toBe(50);
    expect(progressPercent(333, 1000)).toBe(33.3);
  });

  it("clamps above the total", () => {
    expect(progressPercent(1500, 1000)).toBe(100);
  });

  it("returns null when the total size is unknown or zero", () => {
    expect(progressPercent(500, null)).toBe(null);
    expect(progressPercent(500, 0)).toBe(null);
    expect(progressPercent(500, -10)).toBe(null);
  });
});

describe("downloadWindowUi — IDM status rows", () => {
  const task = {
    state: "Downloading",
    total_size: 10_485_760,
    written: 5_242_880,
    speed_bps: 1_048_576,
    eta_secs: 5,
  };

  it("lists state, file size, downloaded with %, rate, time left and resumable", () => {
    const rows = buildStatusRows(task, "ar");
    expect(rows.map((r) => r.key)).toEqual([
      "state",
      "fileSize",
      "downloaded",
      "transferRate",
      "timeLeft",
      "resumable",
    ]);

    const byKey = Object.fromEntries(rows.map((r) => [r.key, r]));
    expect(byKey.state.value).toBe("Downloading");
    expect(byKey.fileSize.value).toBe("10.0 MB");
    expect(byKey.downloaded.value).toBe("5.0 MB (50%)");
    expect(byKey.transferRate.value).toBe("1.0 MB/s");
    expect(byKey.timeLeft.value).toBe("5ث");
    expect(byKey.resumable.value).toBe("نعم");
  });

  it("carries i18n label keys instead of hardcoded labels", () => {
    const rows = buildStatusRows(task, "en");
    expect(rows.find((r) => r.key === "downloaded").labelKey).toBe("downloadWindow.downloaded");
    expect(rows.find((r) => r.key === "timeLeft").labelKey).toBe("downloadWindow.timeLeft");
    expect(rows.find((r) => r.key === "resumable").labelKey).toBe("downloadWindow.resumable");
  });

  it("renders the same formatted values in English", () => {
    const byKey = Object.fromEntries(buildStatusRows(task, "en").map((r) => [r.key, r]));
    expect(byKey.downloaded.value).toBe("5.0 MB (50%)");
    expect(byKey.resumable.value).toBe("Yes");
    expect(byKey.fileSize.valueDir).toBe("ltr");
  });

  it("adds a parts row when the backend reports connections", () => {
    const rows = buildStatusRows({ ...task, parts: 4 }, "ar");
    expect(rows.map((r) => r.key)[1]).toBe("parts");
    expect(rows.find((r) => r.key === "parts").value).toBe("4");
  });

  it("degrades to placeholders for an unknown size and no speed", () => {
    const byKey = Object.fromEntries(
      buildStatusRows({ state: "Queued", written: 2048 }, "ar").map((r) => [r.key, r])
    );
    expect(byKey.fileSize.value).toBe("حجم غير محدد");
    expect(byKey.downloaded.value).toBe("2.0 KB");
    expect(byKey.transferRate.value).toBe("—");
    expect(byKey.timeLeft.value).toBe("—");
    expect(byKey.resumable.value).toBe("لا");
  });

  it("tolerates a null task", () => {
    const rows = buildStatusRows(null, "ar");
    expect(rows.find((r) => r.key === "state").value).toBe("Queued");
    expect(rows.find((r) => r.key === "downloaded").value).toBe("0 B");
  });
});

describe("downloadWindowUi — buildConnectionSegments", () => {
  it("prefers real checkpoint ranges", () => {
    const segments = buildConnectionSegments({
      total_size: 2000,
      segment_ranges: [
        { start: 0, end: 1000, completed: true },
        { start: 1000, end: 2000, completed: false },
      ],
    });

    expect(segments).toEqual([
      { start: 0, end: 1000, written: 1000, total_completed: true },
      { start: 1000, end: 2000, written: 0, total_completed: false },
    ]);
  });

  it("honours a partially written range", () => {
    const segments = buildConnectionSegments({
      segment_ranges: [{ start: 0, end: 1000, written: 250 }],
    });
    expect(segments[0].written).toBe(250);
    expect(segments[0].total_completed).toBe(false);
  });

  it("synthesises equal bands when only parts are known", () => {
    const segments = buildConnectionSegments({ total_size: 1000, written: 500, parts: 4 });
    expect(segments.map((s) => s.start)).toEqual([0, 250, 500, 750]);
    expect(segments.map((s) => s.end)).toEqual([250, 500, 750, 1000]);
    expect(segments.map((s) => s.written)).toEqual([250, 250, 0, 0]);
    expect(segments.map((s) => s.total_completed)).toEqual([true, true, false, false]);
  });

  it("falls back to the `connections` field", () => {
    const segments = buildConnectionSegments({ total_size: 100, connections: 2 });
    expect(segments).toHaveLength(2);
  });

  it("returns nothing when the size or connection count is unknown", () => {
    expect(buildConnectionSegments({ written: 10, parts: 4 })).toEqual([]);
    expect(buildConnectionSegments({ total_size: 100, parts: 0 })).toEqual([]);
    expect(buildConnectionSegments(null)).toEqual([]);
  });

  it("feeds the segmented bar one cell per connection", () => {
    const segments = buildConnectionSegments({ total_size: 1000, written: 500, parts: 4 });
    const bars = buildSegmentBars(segments, { maxCells: segments.length });
    expect(bars).toHaveLength(4);
    expect(bars.map((b) => b.fraction)).toEqual([1, 1, 0, 0]);
  });
});

describe("downloadWindowUi — per-connection table rows", () => {
  it("numbers connections and formats downloaded bytes, range and status", () => {
    const rows = buildConnectionTableRows(
      [
        { start: 0, end: 1024, written: 1024 },
        { start: 1024, end: 2048, written: 512 },
        { start: 2048, end: 3072, written: 0 },
      ],
      "en"
    );

    expect(rows.map((r) => r.index)).toEqual([1, 2, 3]);
    expect(rows.map((r) => r.downloaded)).toEqual(["1.0 KB", "512 B", "0 B"]);
    expect(rows.map((r) => r.status)).toEqual(["done", "downloading", "pending"]);
    expect(rows[0].info).toBe("0 B – 1.0 KB");
  });

  it("returns an empty list without segments", () => {
    expect(buildConnectionTableRows([])).toEqual([]);
    expect(buildConnectionTableRows(null)).toEqual([]);
  });
});

describe("downloadWindowUi — formatRangeInfo", () => {
  it("renders a byte range", () => {
    expect(formatRangeInfo(0, 1024, "en")).toBe("0 B – 1.0 KB");
  });

  it("returns a dash for an empty range", () => {
    expect(formatRangeInfo(5, 5, "ar")).toBe("—");
    expect(formatRangeInfo(10, 5, "ar")).toBe("—");
  });
});

describe("downloadWindowUi — buildSpeedLimitState", () => {
  it("reports a configured limit", () => {
    const state = buildSpeedLimitState({ speed_limit_kbps: 512 }, "ar");
    expect(state.enabled).toBe(true);
    expect(state.limitKbps).toBe(512);
    expect(state.label).toBe("512 KB/s");
  });

  it("reports unlimited when no limit is configured", () => {
    const state = buildSpeedLimitState({}, "ar");
    expect(state.enabled).toBe(false);
    expect(state.limitKbps).toBe(null);
    expect(state.label).toBe("غير محدود");
    expect(buildSpeedLimitState(null, "en").label).toBe("Unlimited");
  });

  it("treats a zero limit as unlimited", () => {
    expect(buildSpeedLimitState({ speed_limit_kbps: 0 }, "en").enabled).toBe(false);
  });
});

describe("downloadWindowUi — getWindowActionFlags", () => {
  it("allows pause, cancel and hide while downloading", () => {
    const flags = getWindowActionFlags({ state: "Downloading" });
    expect(flags).toMatchObject({
      pause: true,
      resume: false,
      retry: false,
      cancel: true,
      hide: true,
      openFile: false,
      reveal: false,
    });
  });

  it("allows resume while paused and retry while failed", () => {
    expect(getWindowActionFlags({ state: "Paused" }).resume).toBe(true);
    const failed = getWindowActionFlags({ state: "Failed" });
    expect(failed.retry).toBe(true);
    expect(failed.resume).toBe(false);
    expect(failed.pause).toBe(false);
  });

  it("disables the transient actions while a start is pending", () => {
    const flags = getWindowActionFlags({ state: "Downloading" }, { isPending: true });
    expect(flags.pause).toBe(false);
    expect(flags.resume).toBe(false);
    expect(flags.cancel).toBe(true);
  });

  it("never offers cancel for a completed task", () => {
    const flags = getWindowActionFlags({ state: "Completed" });
    expect(flags.cancel).toBe(false);
    expect(flags.openFile).toBe(true);
  });
});

describe("downloadWindowUi — buildCompleteDetails", () => {
  it("shows name, size and the saved folder", () => {
    const details = buildCompleteDetails(
      { filename: "file.zip", total_size: 2048, folder: "C:\\Users\\me\\Downloads" },
      "ar"
    );
    expect(details.filename).toBe("file.zip");
    expect(details.size).toBe("2.0 KB");
    expect(details.savedPath).toBe("C:\\Users\\me\\Downloads");
  });

  it("falls back to written bytes when the total is unknown", () => {
    expect(buildCompleteDetails({ filename: "a.bin", written: 1024 }, "en").size).toBe("1.0 KB");
  });

  it("accepts save_path as a folder alias", () => {
    expect(buildCompleteDetails({ filename: "b", save_path: "D:\\x" }, "en").savedPath).toBe("D:\\x");
  });

  it("uses dashes and an unknown-size label for a missing task", () => {
    const details = buildCompleteDetails(null, "ar");
    expect(details.filename).toBe("—");
    expect(details.savedPath).toBe("—");
    expect(details.size).toBe("حجم غير محدد");
  });
});
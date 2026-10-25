import { describe, it, expect } from "vitest";
import {
  TOUR_STORAGE_KEY,
  TOUR_STEPS,
  isTourCompleted,
  setTourCompleted,
  clearTourCompleted,
  getNextStepIndex,
  getPrevStepIndex,
  getRtlKeyAction,
  computeCutoutBounds,
} from "./tour.js";

describe("tour.js pure logic", () => {
  it("defines exactly 6 tour steps with titles and descriptions", () => {
    expect(TOUR_STEPS).toHaveLength(6);
    expect(TOUR_STEPS.map((s) => s.id)).toEqual([
      "add_download",
      "categories_sidebar",
      "search_filter",
      "task_progress",
      "settings_button",
      "browser_extension",
    ]);

    for (const step of TOUR_STEPS) {
      expect(step.titleKey).toMatch(/^tour\.step\dTitle$/);
      expect(step.descKey).toMatch(/^tour\.step\dDesc$/);
      expect(typeof step.selector).toBe("string");
    }
  });

  it("handles next and previous step sequence with boundaries", () => {
    expect(getNextStepIndex(0, 6)).toBe(1);
    expect(getNextStepIndex(4, 6)).toBe(5);
    expect(getNextStepIndex(5, 6)).toBe(5); // clamped at last step

    expect(getPrevStepIndex(5)).toBe(4);
    expect(getPrevStepIndex(1)).toBe(0);
    expect(getPrevStepIndex(0)).toBe(0); // clamped at 0
    expect(getPrevStepIndex(-1)).toBe(0);
  });

  it("sets, reads, and clears completion status in storage", () => {
    const store = new Map();
    const mockStorage = {
      getItem: (k) => (store.has(k) ? store.get(k) : null),
      setItem: (k, v) => store.set(k, String(v)),
      removeItem: (k) => store.delete(k),
    };

    expect(isTourCompleted(mockStorage)).toBe(false);

    setTourCompleted(mockStorage);
    expect(mockStorage.getItem(TOUR_STORAGE_KEY)).toBe("1");
    expect(isTourCompleted(mockStorage)).toBe(true);

    clearTourCompleted(mockStorage);
    expect(mockStorage.getItem(TOUR_STORAGE_KEY)).toBe(null);
    expect(isTourCompleted(mockStorage)).toBe(false);
  });

  it("maps keyboard navigation correctly in RTL and LTR modes", () => {
    // RTL: ArrowLeft is Next, ArrowRight is Prev
    expect(getRtlKeyAction("ArrowLeft", true)).toBe("next");
    expect(getRtlKeyAction("ArrowRight", true)).toBe("prev");

    // LTR: ArrowRight is Next, ArrowLeft is Prev
    expect(getRtlKeyAction("ArrowRight", false)).toBe("next");
    expect(getRtlKeyAction("ArrowLeft", false)).toBe("prev");

    // Escape skips in both
    expect(getRtlKeyAction("Escape", true)).toBe("skip");
    expect(getRtlKeyAction("Escape", false)).toBe("skip");

    // Other keys ignored
    expect(getRtlKeyAction("Enter", true)).toBe(null);
    expect(getRtlKeyAction("KeyA", false)).toBe(null);
  });

  it("computes element cutout bounds when element is present", () => {
    const mockElement = {
      getBoundingClientRect: () => ({
        top: 100,
        left: 200,
        width: 150,
        height: 50,
      }),
    };

    const bounds = computeCutoutBounds("#my-target", () => mockElement, { width: 1200, height: 800 });
    expect(bounds.isFallback).toBe(false);
    expect(bounds.top).toBe(94); // 100 - padding(6)
    expect(bounds.left).toBe(194); // 200 - padding(6)
    expect(bounds.width).toBe(162); // 150 + 12
    expect(bounds.height).toBe(62); // 50 + 12
  });

  it("falls back to centered coordinates when anchor is missing or zero sized", () => {
    // Missing element
    const missingBounds = computeCutoutBounds("#not-found", () => null, { width: 1000, height: 600 });
    expect(missingBounds.isFallback).toBe(true);
    expect(missingBounds.left).toBe(500);
    expect(missingBounds.top).toBe(300);
    expect(missingBounds.width).toBe(0);
    expect(missingBounds.height).toBe(0);

    // Collapsed element (0 width)
    const collapsedElement = {
      getBoundingClientRect: () => ({ top: 10, left: 10, width: 0, height: 0 }),
    };
    const collapsedBounds = computeCutoutBounds("#collapsed", () => collapsedElement, { width: 800, height: 600 });
    expect(collapsedBounds.isFallback).toBe(true);
    expect(collapsedBounds.left).toBe(400);
    expect(collapsedBounds.top).toBe(300);
  });
});

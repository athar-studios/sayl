import { describe, it, expect, beforeEach } from "vitest";
import {
  calculateMedian,
  calculateMax,
  calculateRate,
  shouldThrottle,
  resetPerf,
  addClickSample,
  getClickStats,
  startClick,
  endClick,
} from "./perf.js";

describe("Performance instrumentation pure helpers (perf.js)", () => {
  beforeEach(() => {
    resetPerf();
  });

  describe("calculateMedian", () => {
    it("returns 0 for empty or invalid input", () => {
      expect(calculateMedian([])).toBe(0);
      expect(calculateMedian(null)).toBe(0);
      expect(calculateMedian(undefined)).toBe(0);
    });

    it("calculates median for single item", () => {
      expect(calculateMedian([42.5])).toBe(42.5);
    });

    it("calculates median for odd-length arrays", () => {
      expect(calculateMedian([10, 50, 20])).toBe(20);
      expect(calculateMedian([5, 1, 9, 3, 7])).toBe(5);
    });

    it("calculates median for even-length arrays as average of two middle items", () => {
      expect(calculateMedian([10, 20, 30, 40])).toBe(25);
      expect(calculateMedian([1, 2])).toBe(1.5);
    });
  });

  describe("calculateMax", () => {
    it("returns 0 for empty or invalid input", () => {
      expect(calculateMax([])).toBe(0);
      expect(calculateMax(null)).toBe(0);
    });

    it("returns maximum value in array", () => {
      expect(calculateMax([12, 45.6, 3, 99.1, 50])).toBe(99.1);
    });
  });

  describe("calculateRate", () => {
    it("returns 0 when count or duration is zero or negative", () => {
      expect(calculateRate(0, 1000)).toBe(0);
      expect(calculateRate(10, 0)).toBe(0);
      expect(calculateRate(10, -500)).toBe(0);
      expect(calculateRate(-5, 1000)).toBe(0);
    });

    it("calculates rate per second correctly", () => {
      expect(calculateRate(10, 2000)).toBe(5);
      expect(calculateRate(1, 1000)).toBe(1);
      expect(calculateRate(3, 1500)).toBe(2);
    });
  });

  describe("shouldThrottle", () => {
    it("does not throttle if last report time is 0 or null", () => {
      expect(shouldThrottle(0, 1000, 5000)).toBe(false);
      expect(shouldThrottle(null, 1000, 5000)).toBe(false);
    });

    it("throttles when elapsed time is less than min interval", () => {
      expect(shouldThrottle(1000, 3000, 5000)).toBe(true);
      expect(shouldThrottle(1000, 5999, 5000)).toBe(true);
    });

    it("does not throttle once min interval has passed", () => {
      expect(shouldThrottle(1000, 6000, 5000)).toBe(false);
      expect(shouldThrottle(1000, 7500, 5000)).toBe(false);
    });
  });

  describe("Click sample buffer and getClickStats", () => {
    it("maintains last 20 samples maximum", () => {
      for (let i = 1; i <= 25; i++) {
        addClickSample(i * 10);
      }
      const stats = getClickStats();
      expect(stats.count).toBe(20);
      expect(stats.samples).toHaveLength(20);
      expect(stats.samples[0]).toBe(60);
      expect(stats.samples[19]).toBe(250);
      expect(stats.max).toBe(250);
      expect(stats.median).toBe(155);
    });

    it("startClick returns a valid timestamp and endClick captures sample", () => {
      const start = startClick();
      expect(typeof start).toBe("number");
      expect(start).toBeGreaterThan(0);
      endClick(start, "test-action");
      const stats = getClickStats();
      expect(stats.count).toBeGreaterThanOrEqual(1);
    });
  });
});

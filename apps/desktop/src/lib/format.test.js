import { describe, it, expect } from "vitest";
import {
  formatBytes,
  formatSpeed,
  formatEta,
  formatPercent,
  formatBidi,
  formatLtr,
  formatDate,
  canPause,
  canResume,
  canPauseTask,
  canResumeTask,
} from "./format.js";

describe("Formatting helpers", () => {
  describe("formatBytes", () => {
    it("handles zero and negative bytes", () => {
      expect(formatBytes(0, "en")).toBe("0 B");
      expect(formatBytes(0, "ar")).toBe("0 B");
      expect(formatBytes(-10, "en")).toBe("0 B");
      expect(formatBytes(-10, "ar")).toBe("0 B");
    });

    it("handles null and undefined gracefully", () => {
      expect(formatBytes(null, "en")).toBe("Unknown size");
      expect(formatBytes(undefined, "ar")).toBe("حجم غير محدد");
    });

    it("formats KB, MB, and GB properly in English", () => {
      expect(formatBytes(1024, "en")).toBe("1.0 KB");
      expect(formatBytes(5242880, "en")).toBe("5.0 MB");
      expect(formatBytes(2147483648, "en")).toBe("2.0 GB");
    });

    it("formats KB, MB, and GB properly in Arabic using standard technical units (KB / MB / GB)", () => {
      expect(formatBytes(1024, "ar")).toBe("1.0 KB");
      expect(formatBytes(5242880, "ar")).toBe("5.0 MB");
      expect(formatBytes(2147483648, "ar")).toBe("2.0 GB");
    });

    it("written/total ordering preserves smaller written before larger total", () => {
      const written = formatBytes(72 * 1024 * 1024, "ar");
      const total = formatBytes(100 * 1024 * 1024, "ar");
      expect(written).toBe("72.0 MB");
      expect(total).toBe("100.0 MB");
      const combined = `${written} / ${total}`;
      expect(combined).toBe("72.0 MB / 100.0 MB");
    });
  });

  describe("formatSpeed", () => {
    it("handles zero or empty speeds", () => {
      expect(formatSpeed(0, "en")).toBe("0 B/s");
      expect(formatSpeed(null, "ar")).toBe("0 B/s");
    });

    it("formats speeds with appropriate units in English and Arabic", () => {
      expect(formatSpeed(1024, "en")).toBe("1.0 KB/s");
      expect(formatSpeed(2621440, "en")).toBe("2.5 MB/s");
      expect(formatSpeed(1024, "ar")).toBe("1.0 KB/s");
      expect(formatSpeed(2621440, "ar")).toBe("2.5 MB/s");
      expect(formatSpeed(1073741824, "ar")).toBe("1.0 GB/s");
    });
  });

  describe("formatEta", () => {
    it("returns calculating for zero or negative eta", () => {
      expect(formatEta(0, "en")).toBe("Calculating...");
      expect(formatEta(null, "ar")).toBe("جارٍ الحساب...");
    });

    it("formats seconds, minutes, and hours in English", () => {
      expect(formatEta(45, "en")).toBe("45 s");
      expect(formatEta(150, "en")).toBe("2 m 30 s");
      expect(formatEta(200, "en")).toBe("3 m 20 s");
      expect(formatEta(3600, "en")).toBe("1 h");
      expect(formatEta(3660, "en")).toBe("1 h 1 m");
    });

    it("formats seconds, minutes, and hours in Arabic compact notation without min/m", () => {
      expect(formatEta(45, "ar")).toBe("45ث");
      expect(formatEta(150, "ar")).toBe("2د 30ث");
      expect(formatEta(200, "ar")).toBe("3د 20ث");
      expect(formatEta(3600, "ar")).toBe("1س");
      expect(formatEta(3660, "ar")).toBe("1س 1د");
    });
  });

  describe("formatPercent", () => {
    it("renders percent in Arabic with standard percent symbol % and no parentheses", () => {
      expect(formatPercent(52.5, "ar")).toBe("52.5%");
      expect(formatPercent(100, "ar")).toBe("100%");
      expect(formatPercent(0, "ar")).toBe("0%");
      expect(formatPercent(75.33, "ar")).toBe("75.3%");
    });

    it("renders percent in English with % and no parentheses", () => {
      expect(formatPercent(52.5, "en")).toBe("52.5%");
      expect(formatPercent(100, "en")).toBe("100%");
      expect(formatPercent(0, "en")).toBe("0%");
    });

    it("handles null and undefined gracefully", () => {
      expect(formatPercent(null, "ar")).toBe("");
      expect(formatPercent(undefined, "ar")).toBe("");
      expect(formatPercent(NaN, "ar")).toBe("");
    });

    it("never includes parentheses for any percentage values", () => {
      const values = [0, 50, 72, 72.5, 99.9, 100];
      for (const val of values) {
        const resAr = formatPercent(val, "ar");
        const resEn = formatPercent(val, "en");
        expect(resAr).not.toContain("(");
        expect(resAr).not.toContain(")");
        expect(resEn).not.toContain("(");
        expect(resEn).not.toContain(")");
      }
    });
  });

  describe("RTL safety and bidi isolate", () => {
    it("wraps mixed runs in Unicode isolate characters for RTL safety", () => {
      const wrapped = formatBidi("52.5%");
      expect(wrapped).toBe("\u206852.5%\u2069");
      expect(formatBidi(null)).toBe("");
      expect(formatBidi("")).toBe("");

      const ltrWrapped = formatLtr("14.2 MB/s");
      expect(ltrWrapped).toBe("\u206614.2 MB/s\u2069");
      expect(formatLtr(null)).toBe("");
      expect(formatLtr("")).toBe("");
    });

    it("ensures percent formatting does not include brackets and maintains LTR isolation order", () => {
      const res = formatPercent(52.5, "ar");
      expect(res).not.toContain("(");
      expect(res).not.toContain(")");
      expect(res).toBe("52.5%");
      expect(res.endsWith("%")).toBe(true);
    });
  });

  describe("formatDate", () => {
    it("falls back to — when date is missing or invalid", () => {
      expect(formatDate(null)).toBe("—");
      expect(formatDate(undefined)).toBe("—");
      expect(formatDate("")).toBe("—");
      expect(formatDate("invalid-date-string")).toBe("—");
    });

    it("formats ISO date strings in Arabic and English", () => {
      const iso = "2026-03-29T12:00:00Z";
      const arDate = formatDate(iso, "ar");
      const enDate = formatDate(iso, "en");
      expect(arDate).not.toBe("—");
      expect(enDate).not.toBe("—");
      expect(arDate).toContain("2026");
      expect(enDate).toContain("2026");
    });

    it("formats with ar-EG locale producing correct latn numbers", () => {
      const iso = "2026-10-25T14:48:00Z";
      const arDate = formatDate(iso, "ar");
      expect(arDate).not.toBe("—");
      expect(arDate).toMatch(/2026/);
      expect(arDate).not.toMatch(/[٠-٩]/);
    });
  });

  describe("canPauseTask and canResumeTask enablement", () => {
    it("allows pause for Queued, Probing, Downloading, Verifying, Finalizing", () => {
      const pausable = ["Queued", "Probing", "Downloading", "Verifying", "Finalizing"];
      for (const st of pausable) {
        expect(canPauseTask(st)).toBe(true);
      }
      expect(canPauseTask("Paused")).toBe(false);
      expect(canPauseTask("Completed")).toBe(false);
      expect(canPauseTask("Cancelled")).toBe(false);
    });

    it("allows resume for Paused, Pausing, Failed, Queued, Cancelled", () => {
      const resumable = ["Paused", "Pausing", "Failed", "Queued", "Cancelled"];
      for (const st of resumable) {
        expect(canResumeTask(st)).toBe(true);
      }
      expect(canResumeTask("Downloading")).toBe(false);
      expect(canResumeTask("Completed")).toBe(false);
    });

    it("keeps backward-compatible canPause and canResume definitions", () => {
      expect(canPause("Downloading")).toBe(true);
      expect(canPause("Queued")).toBe(false);
      expect(canResume("Paused")).toBe(true);
      expect(canResume("Downloading")).toBe(false);
    });
  });
});

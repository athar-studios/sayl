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
  isTaskPending,
  getTaskStateKey,
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
    const fixedNow = new Date(2026, 9, 26, 12, 0); // 2026-10-26 12:00 local

    it("falls back to — when date is missing or invalid", () => {
      expect(formatDate(null)).toBe("—");
      expect(formatDate(undefined)).toBe("—");
      expect(formatDate("")).toBe("—");
      expect(formatDate("invalid-date-string")).toBe("—");
    });

    it("formats today's date as اليوم hh:mm م in Arabic and Today h:mm A in English", () => {
      const todayDate = new Date(2026, 9, 26, 14, 48); // today at 14:48
      const arToday = formatDate(todayDate.toISOString(), "ar", fixedNow);
      const enToday = formatDate(todayDate.toISOString(), "en", fixedNow);

      expect(arToday).toMatch(/^اليوم \d{2}:\d{2} [صم]$/);
      expect(arToday).toContain("اليوم");
      expect(arToday).toContain("02:48 م");
      expect(enToday).toMatch(/^Today \d{1,2}:\d{2} [AP]M$/);
      expect(enToday).toContain("Today");
      expect(enToday).toContain("2:48 PM");
    });

    it("formats yesterday's date as أمس hh:mm م in Arabic and Yesterday h:mm A in English", () => {
      const yesterdayDate = new Date(2026, 9, 25, 9, 15); // yesterday at 09:15
      const arYesterday = formatDate(yesterdayDate.toISOString(), "ar", fixedNow);
      const enYesterday = formatDate(yesterdayDate.toISOString(), "en", fixedNow);

      expect(arYesterday).toMatch(/^أمس \d{2}:\d{2} [صم]$/);
      expect(arYesterday).toContain("أمس");
      expect(arYesterday).toContain("09:15 ص");
      expect(enYesterday).toMatch(/^Yesterday \d{1,2}:\d{2} [AP]M$/);
      expect(enYesterday).toContain("Yesterday");
      expect(enYesterday).toContain("9:15 AM");
    });

    it("formats older dates as D MMMM YYYY (e.g., 25 أكتوبر 2026 / Oct 25, 2026)", () => {
      const olderDate = new Date(2026, 9, 20, 10, 0); // 20 Oct 2026
      const arOlder = formatDate(olderDate.toISOString(), "ar", fixedNow);
      const enOlder = formatDate(olderDate.toISOString(), "en", fixedNow);

      expect(arOlder).toBe("20 أكتوبر 2026");
      expect(enOlder).toBe("Oct 20, 2026");
    });

    it("formats with ar-EG locale producing correct latn numbers", () => {
      const olderDate = new Date(2026, 9, 25, 14, 48);
      const nowNextMonth = new Date(2026, 10, 15, 12, 0);
      const arDate = formatDate(olderDate.toISOString(), "ar", nowNextMonth);
      expect(arDate).not.toBe("—");
      expect(arDate).toMatch(/2026/);
      expect(arDate).not.toMatch(/[٠-٩]/);
      expect(arDate).toBe("25 أكتوبر 2026");
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

    it("evaluates isTaskPending correctly across Set, Array, and Object inputs", () => {
      const set = new Set(["task-1", "task-2"]);
      expect(isTaskPending(set, "task-1")).toBe(true);
      expect(isTaskPending(set, "task-3")).toBe(false);

      const arr = ["task-1", "task-2"];
      expect(isTaskPending(arr, "task-2")).toBe(true);
      expect(isTaskPending(arr, "task-3")).toBe(false);

      const map = { "task-1": true, "task-2": false };
      expect(isTaskPending(map, "task-1")).toBe(true);
      expect(isTaskPending(map, "task-2")).toBe(false);
      expect(isTaskPending(map, "task-3")).toBe(false);

      expect(isTaskPending(null, "task-1")).toBe(false);
      expect(isTaskPending(undefined, "task-1")).toBe(false);
      expect(isTaskPending(set, null)).toBe(false);
    });

    it("returns correct translation key with getTaskStateKey for pending and normal states", () => {
      expect(getTaskStateKey("Paused", true)).toBe("task.connecting");
      expect(getTaskStateKey("Downloading", true)).toBe("task.connecting");
      expect(getTaskStateKey("Failed", true)).toBe("task.connecting");

      expect(getTaskStateKey("Paused", false)).toBe("state.Paused");
      expect(getTaskStateKey("Downloading", false)).toBe("state.Downloading");
      expect(getTaskStateKey("Completed", false)).toBe("state.Completed");
    });
  });
});

import { describe, it, expect } from "vitest";
import {
  areAllDownloadsFinished,
  getBlockedReasonMessage,
  getBlockedReasonShort,
  BLOCKED_REASONS,
} from "./shutdown.js";

describe("Shutdown decisions & blocked reasons (shutdown.js)", () => {
  describe("areAllDownloadsFinished evaluation", () => {
    it("returns false when task list is empty or not an array", () => {
      expect(areAllDownloadsFinished([])).toBe(false);
      expect(areAllDownloadsFinished(null)).toBe(false);
      expect(areAllDownloadsFinished(undefined)).toBe(false);
    });

    it("returns false when any task is active / running", () => {
      const activeStates = ["Queued", "Probing", "Downloading", "Verifying", "Finalizing"];
      for (const st of activeStates) {
        const tasks = [
          { state: "Completed" },
          { state: st },
          { state: "Cancelled" },
        ];
        expect(areAllDownloadsFinished(tasks)).toBe(false);
      }
    });

    it("returns false when any task is paused or pausing", () => {
      expect(areAllDownloadsFinished([{ state: "Completed" }, { state: "Paused" }])).toBe(false);
      expect(areAllDownloadsFinished([{ state: "Cancelled" }, { state: "Pausing" }])).toBe(false);
    });

    it("returns false when any task is failed", () => {
      expect(areAllDownloadsFinished([{ state: "Completed" }, { state: "Failed" }])).toBe(false);
    });

    it("returns false when any task is cancelling", () => {
      expect(areAllDownloadsFinished([{ state: "Cancelling" }, { state: "Completed" }])).toBe(false);
    });

    it("returns true only when every task is Completed or Cancelled", () => {
      expect(areAllDownloadsFinished([{ state: "Completed" }])).toBe(true);
      expect(areAllDownloadsFinished([{ state: "Cancelled" }])).toBe(true);
      expect(
        areAllDownloadsFinished([
          { state: "Completed" },
          { state: "Cancelled" },
          { state: "Completed" },
        ])
      ).toBe(true);
    });
  });

  describe("Blocked reason codes mapping", () => {
    it("maps all 4 documented blocked reasons", () => {
      const codes = ["failed_download", "paused_download", "active_download", "no_tasks"];
      for (const code of codes) {
        expect(BLOCKED_REASONS[code]).toBeDefined();
        const msgAr = getBlockedReasonMessage(code, "ar");
        const msgEn = getBlockedReasonMessage(code, "en");
        expect(msgAr).toBeTruthy();
        expect(msgEn).toBeTruthy();
      }
    });

    it("provides correct Arabic short labels matching specifications", () => {
      expect(getBlockedReasonShort("failed_download", "ar")).toBe("فشل");
      expect(getBlockedReasonShort("paused_download", "ar")).toBe("متوقف");
      expect(getBlockedReasonShort("active_download", "ar")).toBe("قيد التنزيل");
      expect(getBlockedReasonShort("no_tasks", "ar")).toBe("لا توجد تنزيلات");
    });
  });
});

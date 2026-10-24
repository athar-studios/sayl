import { describe, it, expect } from "vitest";
import { formatBytes, formatSpeed, formatEta } from "./format.js";

describe("Formatting helpers", () => {
  describe("formatBytes", () => {
    it("handles zero and negative bytes", () => {
      expect(formatBytes(0, "en")).toBe("0 B");
      expect(formatBytes(0, "ar")).toBe("0 بايت");
      expect(formatBytes(-10, "en")).toBe("0 B");
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

    it("formats KB, MB, and GB properly in Arabic", () => {
      expect(formatBytes(1024, "ar")).toBe("1.0 ك.ب");
      expect(formatBytes(5242880, "ar")).toBe("5.0 م.ب");
      expect(formatBytes(2147483648, "ar")).toBe("2.0 ج.ب");
    });
  });

  describe("formatSpeed", () => {
    it("handles zero or empty speeds", () => {
      expect(formatSpeed(0, "en")).toBe("0 B/s");
      expect(formatSpeed(null, "ar")).toBe("0 بايت/ث");
    });

    it("formats speeds with appropriate units", () => {
      expect(formatSpeed(1024, "en")).toBe("1.0 KB/s");
      expect(formatSpeed(2621440, "en")).toBe("2.5 MB/s");
      expect(formatSpeed(2621440, "ar")).toBe("2.5 م.ب/ث");
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
      expect(formatEta(3600, "en")).toBe("1 h");
      expect(formatEta(3660, "en")).toBe("1 h 1 m");
    });

    it("formats seconds, minutes, and hours in Arabic", () => {
      expect(formatEta(45, "ar")).toBe("45 ث");
      expect(formatEta(150, "ar")).toBe("2 د 30 ث");
      expect(formatEta(3600, "ar")).toBe("1 س");
      expect(formatEta(3660, "ar")).toBe("1 س 1 د");
    });
  });
});

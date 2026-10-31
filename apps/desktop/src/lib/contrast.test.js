import { describe, it, expect } from "vitest";

/**
 * Converts a 6-character hex string (e.g. "#1f2937") to RGB components [0..255].
 * @param {string} hex
 * @returns {[number, number, number]}
 */
function hexToRgb(hex) {
  const clean = hex.replace("#", "").trim();
  const r = parseInt(clean.substring(0, 2), 16);
  const g = parseInt(clean.substring(2, 4), 16);
  const b = parseInt(clean.substring(4, 6), 16);
  return [r, g, b];
}

/**
 * Calculates WCAG 2.1 relative luminance for an RGB tuple.
 * Formula: https://www.w3.org/TR/WCAG21/#dfn-relative-luminance
 * @param {[number, number, number]} rgb
 * @returns {number}
 */
function getLuminance([r, g, b]) {
  const [rs, gs, bs] = [r, g, b].map((val) => {
    const s = val / 255;
    return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
  });
  return 0.2126 * rs + 0.7152 * gs + 0.0722 * bs;
}

/**
 * Calculates WCAG 2.1 contrast ratio between two hex colors.
 * Formula: (L1 + 0.05) / (L2 + 0.05) where L1 is the lighter color.
 * @param {string} hex1
 * @param {string} hex2
 * @returns {number}
 */
export function getContrastRatio(hex1, hex2) {
  const lum1 = getLuminance(hexToRgb(hex1));
  const lum2 = getLuminance(hexToRgb(hex2));
  const l1 = Math.max(lum1, lum2);
  const l2 = Math.min(lum1, lum2);
  return (l1 + 0.05) / (l2 + 0.05);
}

describe("WCAG Contrast Ratio Automated Verification (Item 15)", () => {
  const WCAG_AA_NORMAL_TEXT = 4.5;

  describe("Light Mode Contrast (>= 4.5:1)", () => {
    it("primary text #1f2937 on background #ffffff exceeds 4.5:1", () => {
      const ratio = getContrastRatio("#1f2937", "#ffffff");
      expect(ratio).toBeGreaterThanOrEqual(WCAG_AA_NORMAL_TEXT);
    });

    it("muted text #4b5563 on background #ffffff exceeds 4.5:1", () => {
      const ratio = getContrastRatio("#4b5563", "#ffffff");
      expect(ratio).toBeGreaterThanOrEqual(WCAG_AA_NORMAL_TEXT);
    });
  });

  describe("Dark Mode Contrast (>= 4.5:1)", () => {
    it("primary text #f3f4f6 on card background #1f2937 exceeds 4.5:1", () => {
      const ratio = getContrastRatio("#f3f4f6", "#1f2937");
      expect(ratio).toBeGreaterThanOrEqual(WCAG_AA_NORMAL_TEXT);
    });

    it("primary text #f3f4f6 on main background #111827 exceeds 4.5:1", () => {
      const ratio = getContrastRatio("#f3f4f6", "#111827");
      expect(ratio).toBeGreaterThanOrEqual(WCAG_AA_NORMAL_TEXT);
    });

    it("muted text #d1d5db on card background #1f2937 exceeds 4.5:1", () => {
      const ratio = getContrastRatio("#d1d5db", "#1f2937");
      expect(ratio).toBeGreaterThanOrEqual(WCAG_AA_NORMAL_TEXT);
    });

    it("muted text #d1d5db on main background #111827 exceeds 4.5:1", () => {
      const ratio = getContrastRatio("#d1d5db", "#111827");
      expect(ratio).toBeGreaterThanOrEqual(WCAG_AA_NORMAL_TEXT);
    });
  });
});

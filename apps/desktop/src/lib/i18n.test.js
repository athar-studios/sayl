import { describe, it, expect, beforeEach } from "vitest";
import { get } from "svelte/store";
import { currentLang, setLanguage, toggleLanguage, resolveTranslation } from "./i18n.js";
import ar from "../i18n/ar.js";
import en from "../i18n/en.js";

function getLeafKeys(obj, prefix = "") {
  let keys = [];
  for (const [k, v] of Object.entries(obj)) {
    const full = prefix ? `${prefix}.${k}` : k;
    if (v && typeof v === "object" && !Array.isArray(v)) {
      keys.push(...getLeafKeys(v, full));
    } else {
      keys.push(full);
    }
  }
  return keys.sort();
}

describe("i18n module", () => {
  beforeEach(() => {
    setLanguage("ar");
  });

  it("maintains exact key parity between Arabic and English dictionaries", () => {
    const arKeys = getLeafKeys(ar);
    const enKeys = getLeafKeys(en);
    expect(arKeys).toEqual(enKeys);
  });

  it("defaults to Arabic language", () => {
    expect(get(currentLang)).toBe("ar");
  });

  it("resolves Arabic translations properly", () => {
    expect(resolveTranslation("ar", "app.name")).toBe("سيل");
    expect(resolveTranslation("ar", "toolbar.add")).toBe("إضافة تنزيل");
    expect(resolveTranslation("ar", "state.Downloading")).toBe("جارٍ التنزيل");
    expect(resolveTranslation("ar", "state.Completed")).toBe("مكتمل");
  });

  it("resolves English translations properly", () => {
    expect(resolveTranslation("en", "app.name")).toBe("SAYL");
    expect(resolveTranslation("en", "toolbar.add")).toBe("Add Download");
    expect(resolveTranslation("en", "state.Downloading")).toBe("Downloading");
    expect(resolveTranslation("en", "state.Completed")).toBe("Completed");
  });

  it("toggles language between ar and en", () => {
    expect(get(currentLang)).toBe("ar");
    toggleLanguage();
    expect(get(currentLang)).toBe("en");
    toggleLanguage();
    expect(get(currentLang)).toBe("ar");
  });

  it("handles non-existent translation keys gracefully by returning key path", () => {
    expect(resolveTranslation("ar", "non.existent.key")).toBe("non.existent.key");
  });

  it("sanitizes unsupported language codes to ar", () => {
    setLanguage("invalid_lang");
    expect(get(currentLang)).toBe("ar");
  });
});

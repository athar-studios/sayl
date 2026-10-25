import { describe, it, expect } from "vitest";
import { DEFAULT_SETTINGS, SETTINGS_KEYS, SETTINGS_SECTIONS, normalizeSettings } from "./settings.js";

describe("Settings contract & defaults (settings.js)", () => {
  it("has exactly the 13 documented keys in SETTINGS_KEYS", () => {
    expect(SETTINGS_KEYS).toHaveLength(13);
    expect(SETTINGS_KEYS).toEqual([
      "download_dir",
      "max_concurrent_tasks",
      "max_connections_per_task",
      "theme",
      "language",
      "view_mode",
      "sidebar_collapsed",
      "prevent_sleep",
      "show_download_window",
      "close_download_window_on_finish",
      "close_action",
      "run_at_startup",
      "close_notice_shown",
    ]);
  });

  it("default settings object contains exactly the 13 documented keys", () => {
    const keys = Object.keys(DEFAULT_SETTINGS).sort();
    const expectedKeys = [...SETTINGS_KEYS].sort();
    expect(keys).toEqual(expectedKeys);
  });

  it("guards the documented default values matching the Rust contract", () => {
    expect(DEFAULT_SETTINGS.max_concurrent_tasks).toBe(3);
    expect(DEFAULT_SETTINGS.max_connections_per_task).toBe(4);
    expect(DEFAULT_SETTINGS.theme).toBe("system");
    expect(DEFAULT_SETTINGS.language).toBe("ar");
    expect(DEFAULT_SETTINGS.view_mode).toBe("table");
    expect(DEFAULT_SETTINGS.sidebar_collapsed).toBe(false);
    expect(DEFAULT_SETTINGS.prevent_sleep).toBe(true);
    expect(DEFAULT_SETTINGS.show_download_window).toBe(true);
    expect(DEFAULT_SETTINGS.close_download_window_on_finish).toBe(false);
    expect(DEFAULT_SETTINGS.close_action).toBe("background");
    expect(DEFAULT_SETTINGS.run_at_startup).toBe(false);
    expect(DEFAULT_SETTINGS.close_notice_shown).toBe(false);
  });

  it("normalizeSettings populates missing keys with valid defaults", () => {
    const normalized = normalizeSettings({});
    expect(normalized).toEqual(DEFAULT_SETTINGS);

    const partial = normalizeSettings({
      view_mode: "table",
      sidebar_collapsed: true,
      prevent_sleep: false,
    });
    expect(partial.view_mode).toBe("table");
    expect(partial.sidebar_collapsed).toBe(true);
    expect(partial.prevent_sleep).toBe(false);
    expect(partial.show_download_window).toBe(true);
    expect(partial.close_download_window_on_finish).toBe(false);
    expect(partial.close_action).toBe("background");
    expect(partial.run_at_startup).toBe(false);
    expect(partial.close_notice_shown).toBe(false);
  });

  it("rejects invalid view_mode in normalizeSettings and falls back to 'table'", () => {
    const res = normalizeSettings({ view_mode: "invalid_view" });
    expect(res.view_mode).toBe("table");
  });

  it("preserves and normalizes close_action, run_at_startup, and close_notice_shown", () => {
    const res = normalizeSettings({
      close_action: "quit",
      run_at_startup: true,
      close_notice_shown: true,
    });
    expect(res.close_action).toBe("quit");
    expect(res.run_at_startup).toBe(true);
    expect(res.close_notice_shown).toBe(true);

    const invalid = normalizeSettings({
      close_action: "unknown_action",
      run_at_startup: "not_a_bool",
      close_notice_shown: 123,
    });
    expect(invalid.close_action).toBe("background");
    expect(invalid.run_at_startup).toBe(false);
    expect(invalid.close_notice_shown).toBe(false);
  });

  it("preserves unknown backend fields when normalizing", () => {
    const custom = normalizeSettings({
      custom_backend_field: "sayl-extension-v1",
      extra_num: 42,
    });
    expect(custom.custom_backend_field).toBe("sayl-extension-v1");
    expect(custom.extra_num).toBe(42);
    expect(custom.view_mode).toBe("table");
  });

  it("settings sections are all reachable and have distinct IDs", () => {
    expect(SETTINGS_SECTIONS).toBeDefined();
    expect(SETTINGS_SECTIONS.length).toBe(8);
    const ids = SETTINGS_SECTIONS.map((s) => s.id);
    const uniqueIds = new Set(ids);
    expect(uniqueIds.size).toBe(ids.length);
    expect(ids).toEqual([
      "general",
      "downloads",
      "categories",
      "notifications",
      "power",
      "appearance",
      "extension",
      "about",
    ]);
    for (const sec of SETTINGS_SECTIONS) {
      expect(sec.labelKey).toBeTruthy();
      expect(typeof sec.id).toBe("string");
    }
  });

  it("preserves language and other settings when theme changes", () => {
    const base = { ...DEFAULT_SETTINGS, language: "en", theme: "system" };
    const changedTheme = normalizeSettings({ ...base, theme: "dark" });
    expect(changedTheme.language).toBe("en");
    expect(changedTheme.theme).toBe("dark");

    const changedLang = normalizeSettings({ ...changedTheme, language: "ar" });
    expect(changedLang.language).toBe("ar");
    expect(changedLang.theme).toBe("dark");
  });
});

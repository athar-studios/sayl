import { describe, it, expect } from "vitest";
import { DEFAULT_SETTINGS, SETTINGS_KEYS, normalizeSettings } from "./settings.js";

describe("Settings contract & defaults (settings.js)", () => {
  it("has exactly the 10 documented keys in SETTINGS_KEYS", () => {
    expect(SETTINGS_KEYS).toHaveLength(10);
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
    ]);
  });

  it("default settings object contains exactly the 10 documented keys", () => {
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
  });

  it("rejects invalid view_mode in normalizeSettings and falls back to 'table'", () => {
    const res = normalizeSettings({ view_mode: "invalid_view" });
    expect(res.view_mode).toBe("table");
  });
});

/**
 * Pure settings definition and normalization module.
 * Conforms strictly to the Tauri backend Settings struct contract (10 snake_case keys).
 */

export const DEFAULT_SETTINGS = {
  download_dir: "",
  max_concurrent_tasks: 3,
  max_connections_per_task: 4,
  theme: "system",
  language: "ar",
  view_mode: "table",
  sidebar_collapsed: false,
  prevent_sleep: true,
  show_download_window: true,
  close_download_window_on_finish: false,
};

export const SETTINGS_KEYS = [
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
];

/**
 * Merges partial or backend settings with DEFAULT_SETTINGS, ensuring all 10 keys are present.
 *
 * @param {object|null|undefined} settings
 * @returns {typeof DEFAULT_SETTINGS}
 */
export function normalizeSettings(settings = {}) {
  const s = settings && typeof settings === "object" ? settings : {};

  return {
    download_dir: typeof s.download_dir === "string" ? s.download_dir : DEFAULT_SETTINGS.download_dir,
    max_concurrent_tasks:
      typeof s.max_concurrent_tasks === "number" && s.max_concurrent_tasks >= 1
        ? s.max_concurrent_tasks
        : DEFAULT_SETTINGS.max_concurrent_tasks,
    max_connections_per_task:
      typeof s.max_connections_per_task === "number" && s.max_connections_per_task >= 1
        ? s.max_connections_per_task
        : DEFAULT_SETTINGS.max_connections_per_task,
    theme: ["system", "dark", "light"].includes(s.theme) ? s.theme : DEFAULT_SETTINGS.theme,
    language: ["ar", "en"].includes(s.language) ? s.language : DEFAULT_SETTINGS.language,
    view_mode: ["cards", "table"].includes(s.view_mode) ? s.view_mode : DEFAULT_SETTINGS.view_mode,
    sidebar_collapsed:
      typeof s.sidebar_collapsed === "boolean"
        ? s.sidebar_collapsed
        : DEFAULT_SETTINGS.sidebar_collapsed,
    prevent_sleep:
      typeof s.prevent_sleep === "boolean"
        ? s.prevent_sleep
        : DEFAULT_SETTINGS.prevent_sleep,
    show_download_window:
      typeof s.show_download_window === "boolean"
        ? s.show_download_window
        : DEFAULT_SETTINGS.show_download_window,
    close_download_window_on_finish:
      typeof s.close_download_window_on_finish === "boolean"
        ? s.close_download_window_on_finish
        : DEFAULT_SETTINGS.close_download_window_on_finish,
  };
}

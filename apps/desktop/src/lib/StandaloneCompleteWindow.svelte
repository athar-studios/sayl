<script>
  import { onMount } from "svelte";
  import { safeInvoke } from "./tauri.js";
  import { DEFAULT_SETTINGS, normalizeSettings } from "./settings.js";
  import { setLanguage } from "./i18n.js";
  import CompleteWindow from "./CompleteWindow.svelte";

  export let taskId;

  let task = null;
  let settings = { ...DEFAULT_SETTINGS };

  async function closeNativeWindow() {
    try {
      if (typeof window !== "undefined" && (window.__TAURI_INTERNALS__ || window.__TAURI__)) {
        const { getCurrentWebviewWindow } = await import("@tauri-apps/api/webviewWindow");
        const win = getCurrentWebviewWindow();
        if (win) {
          await win.close();
          return;
        }
      }
    } catch {}
    try {
      await safeInvoke("close_complete_window", { id: taskId });
    } catch {}
  }

  /**
   * «لا تعرض هذه النافذة مرة أخرى»: persists the Rust-backed setting
   * `show_download_complete_window`, so the host stops opening this window.
   */
  async function handleDontShowAgain() {
    settings = { ...settings, show_download_complete_window: false };
    try {
      await safeInvoke("set_settings", { settings });
    } catch (e) {
      console.warn("Could not persist show_download_complete_window:", e);
    }
  }

  function applyActiveTheme(theme) {
    if (theme === "dark") {
      document.documentElement.setAttribute("data-theme", "dark");
    } else if (theme === "light") {
      document.documentElement.setAttribute("data-theme", "light");
    } else {
      const isOsDark =
        typeof window !== "undefined" &&
        window.matchMedia &&
        window.matchMedia("(prefers-color-scheme: dark)").matches;
      document.documentElement.setAttribute("data-theme", isOsDark ? "dark" : "light");
    }
  }

  onMount(async () => {
    try {
      const s = await safeInvoke("get_settings");
      if (s) {
        settings = normalizeSettings(s);
        if (s.language) setLanguage(s.language);
        applyActiveTheme(s.theme);
      } else {
        applyActiveTheme("system");
      }

      const tasks = await safeInvoke("list_tasks");
      if (Array.isArray(tasks)) {
        const found = tasks.find((t) => t.id === taskId);
        if (found) {
          task = found;
          document.title =
            settings.language === "en" ? "Download complete" : "اكتمل التحميل";
        }
      }

      if (typeof window !== "undefined" && (window.__TAURI_INTERNALS__ || window.__TAURI__)) {
        const { getCurrentWebviewWindow } = await import("@tauri-apps/api/webviewWindow");
        const win = getCurrentWebviewWindow();
        if (win) {
          await win.show();
          await win.setFocus();
        }
      }
    } catch (e) {
      console.warn("Standalone completion window load error:", e);
    }
  });
</script>

<main class="standalone-complete-container">
  {#if task}
    <CompleteWindow
      {task}
      on:close={closeNativeWindow}
      on:reveal={(e) => safeInvoke("reveal_task", { id: e.detail })}
      on:openFile={(e) => safeInvoke("open_task_file", { id: e.detail })}
      on:dontShowAgain={handleDontShowAgain}
    />
  {:else}
    <div class="loading-state"><span>...</span></div>
  {/if}
</main>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    overflow: hidden;
    background: var(--color-bg);
    color: var(--color-text);
  }

  .standalone-complete-container {
    width: 100vw;
    height: 100vh;
    display: flex;
    background: var(--color-bg);
    color: var(--color-text);
  }

  .loading-state {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    color: var(--color-text-muted);
  }
</style>

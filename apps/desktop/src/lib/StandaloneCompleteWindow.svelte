<script>
  import { onMount } from "svelte";
  import { safeInvoke } from "./tauri.js";
  import { DEFAULT_SETTINGS, normalizeSettings } from "./settings.js";
  import { setLanguage } from "./i18n.js";
  import CompleteWindow from "./CompleteWindow.svelte";

  export let taskId;

  let task = null;
  let settings = { ...DEFAULT_SETTINGS };
  /** True while «فتح» / «فتح المجلد» is still running; that run closes the window. */
  let commandInFlight = false;
  /** True once the webview has been asked to close, so it is never asked twice. */
  let closing = false;

  /**
   * Closes this native window: the webview first, then the Rust-side
   * `close_complete_window` as a fallback when the webview API is unavailable.
   */
  async function closeNativeWindow() {
    if (closing) return;
    closing = true;
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
      await safeInvoke("close_complete_window", { id: taskId }, { suppressToast: true });
    } catch {}
  }

  /**
   * «فتح» / «فتح المجلد»: run the Rust command and close the window once it
   * settles — including when it fails, so a dead window is never left open.
   * `commandInFlight` is set synchronously, before the first `await`, so the
   * `close` event that CompleteWindow emits right after the action cannot tear
   * the webview down before the command resolves.
   */
  async function runCommand(command, id) {
    const taskIdToUse = id || taskId;
    commandInFlight = true;
    try {
      await safeInvoke(command, { id: taskIdToUse }, { suppressToast: true });
    } catch {
      // The Rust side reports its own failure; the window still closes.
    } finally {
      commandInFlight = false;
      await closeNativeWindow();
    }
  }

  function handleCloseEvent() {
    if (commandInFlight) return;
    closeNativeWindow();
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
      on:close={handleCloseEvent}
      on:reveal={(e) => runCommand("reveal_task", e.detail)}
      on:openFile={(e) => runCommand("open_task_file", e.detail)}
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

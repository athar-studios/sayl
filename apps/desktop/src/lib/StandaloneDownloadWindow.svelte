<script>
  import { onMount, onDestroy } from "svelte";
  import { safeInvoke } from "./tauri.js";
  import { DEFAULT_SETTINGS, normalizeSettings } from "./settings.js";
  import { setLanguage } from "./i18n.js";
  import DownloadWindow from "./DownloadWindow.svelte";

  export let taskId;

  let task = null;
  let settings = { ...DEFAULT_SETTINGS };
  let autoShutdown = false;
  let unlistenProgress = null;
  let unlistenTasksChanged = null;

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
      await safeInvoke("close_download_window", { id: taskId });
    } catch {}
  }

  function handleTaskUpdate(updatedTask) {
    if (!updatedTask) return;
    task = { ...task, ...updatedTask };
    if (task.state === "Cancelled") {
      closeNativeWindow();
    } else if (task.state === "Completed" && settings.close_download_window_on_finish) {
      setTimeout(() => closeNativeWindow(), 500);
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
          if (found.filename) {
            document.title = found.filename;
          }
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

      if (typeof window !== "undefined" && (window.__TAURI_INTERNALS__ || window.__TAURI__)) {
        const { listen } = await import("@tauri-apps/api/event");
        unlistenProgress = await listen("sayl://progress", (event) => {
          const payload = event.payload;
          if (payload && payload.id === taskId && task) {
            const eta =
              payload.speed_bps > 0 && payload.total && payload.total > payload.written
                ? Math.round((payload.total - payload.written) / payload.speed_bps)
                : null;
            task = {
              ...task,
              written: payload.written,
              total_size: payload.total !== null ? payload.total : task.total_size,
              speed_bps: payload.speed_bps,
              eta_secs: eta,
              state: task.state === "Queued" || task.state === "Probing" ? "Downloading" : task.state,
            };
          }
        });

        unlistenTasksChanged = await listen("sayl://tasks-changed", (event) => {
          const payload = event.payload;
          if (!payload) return;
          if (payload.full && Array.isArray(payload.upsert)) {
            const found = payload.upsert.find((t) => t.id === taskId);
            if (found) handleTaskUpdate(found);
          } else if (Array.isArray(payload.upsert)) {
            const found = payload.upsert.find((t) => t.id === taskId);
            if (found) handleTaskUpdate(found);
          }
          if (Array.isArray(payload.removed) && payload.removed.includes(taskId)) {
            closeNativeWindow();
          }
        });
      }
    } catch (e) {
      console.warn("Standalone window load error:", e);
    }
  });

  onDestroy(() => {
    if (unlistenProgress) unlistenProgress();
    if (unlistenTasksChanged) unlistenTasksChanged();
  });
</script>

<main class="standalone-window-container">
  {#if task}
    <DownloadWindow
      {task}
      {autoShutdown}
      autoClose={settings.close_download_window_on_finish}
      maxConnections={settings.max_connections_per_task}
      isNativeWindow={true}
      on:close={closeNativeWindow}
      on:pause={(e) => safeInvoke("pause_task", { id: e.detail })}
      on:resume={(e) => safeInvoke("start_task", { id: e.detail })}
      on:retry={(e) => safeInvoke("start_task", { id: e.detail })}
      on:cancel={async (e) => {
        await safeInvoke("cancel_task", { id: e.detail });
        closeNativeWindow();
      }}
      on:reveal={(e) => safeInvoke("reveal_task", { id: e.detail })}
      on:openFile={(e) => safeInvoke("open_task_file", { id: e.detail })}
      on:toggleAutoShutdown={(e) => (autoShutdown = e.detail)}
      on:toggleAutoClose={async (e) => {
        settings.close_download_window_on_finish = e.detail;
        await safeInvoke("set_settings", { settings });
      }}
    />
  {:else}
    <div class="loading-state">
      <span>...</span>
    </div>
  {/if}
</main>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    overflow: hidden;
    background: var(--color-bg, #f5f8fc);
  }

  .standalone-window-container {
    width: 100vw;
    height: 100vh;
    display: flex;
    align-items: stretch;
    justify-content: stretch;
    box-sizing: border-box;
    background: var(--color-bg, #f5f8fc);
  }

  .standalone-window-container :global(.download-window) {
    width: 100%;
    height: 100%;
    max-width: none;
    max-height: none;
    border-radius: 0;
    border: none;
    box-shadow: none;
    box-sizing: border-box;
    padding: 1rem;
  }

  .loading-state {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    color: #4b5563;
    font-size: 14px;
  }
</style>

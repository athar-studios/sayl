<script>
  import { onMount, onDestroy } from "svelte";
  import { safeInvoke } from "./tauri.js";
  import { DEFAULT_SETTINGS, normalizeSettings } from "./settings.js";
  import { setLanguage, t, currentLang } from "./i18n.js";
  import { friendlyError } from "./errors.js";
  import DownloadWindow from "./DownloadWindow.svelte";
  import {
    mergeProgressPayload,
    findTaskById,
    resolveStandaloneView,
  } from "./downloadWindowUi.js";

  export let taskId;

  let task = null;
  let settings = { ...DEFAULT_SETTINGS };
  let autoShutdown = false;
  let unlistenProgress = null;
  let unlistenTasksChanged = null;
  /** True while «فتح» / «فتح المجلد» is still running; that run closes the window. */
  let commandInFlight = false;
  /** True once the webview has been asked to close, so it is never asked twice. */
  let closing = false;

  // Initial load tracking & error state
  let loadError = null;
  let loadTimedOut = false;
  let retrying = false;
  let pollTimer = null;
  let timeoutTimer = null;

  const POLL_INTERVAL_MS = 200;
  const LOAD_TIMEOUT_MS = 8000;

  $: viewMode = resolveStandaloneView({
    task,
    loadError,
    timedOut: loadTimedOut,
  });

  $: isFailedTask = task?.state === "Failed";
  $: errInfo = isFailedTask || loadError
    ? friendlyError(task || { error: loadError, error_kind: "unknown" }, $currentLang)
    : null;
  $: displayedErrorMessage =
    task?.error || loadError || errInfo?.hint || $t("state.Failed");

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
      await safeInvoke("close_download_window", { id: taskId }, { suppressToast: true });
    } catch {}
  }

  /**
   * «فتح» / «فتح المجلد» once this window has switched to the completion layout:
   * run the Rust command, then close — including when the command fails, so the
   * window never stays open as a dead end. `commandInFlight` is set before the
   * first `await` so the `close` event emitted right after the action is ignored
   * instead of tearing the webview down mid-command.
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

  function handleTaskUpdate(updatedTask) {
    if (!updatedTask) return;
    task = { ...task, ...updatedTask };
    if (task.filename) {
      document.title = task.filename;
    }
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

  /**
   * Attempts to load the task directly via `get_task`, falling back to `list_tasks`.
   * Returns true if a task was successfully found and assigned.
   */
  async function fetchTaskState() {
    if (!taskId) return false;
    try {
      // Direct single-task lookup
      let direct = null;
      try {
        direct = await safeInvoke("get_task", { id: taskId }, { suppressToast: true });
      } catch (e) {
        // Fallback to list_tasks if get_task is unavailable
      }

      if (direct && typeof direct === "object" && direct.id === taskId) {
        task = task ? { ...task, ...direct } : direct;
        if (direct.filename) document.title = direct.filename;
        loadError = null;
        loadTimedOut = false;
        return true;
      }

      // Fallback: list all tasks and locate taskId
      const list = await safeInvoke("list_tasks", {}, { suppressToast: true });
      const found = findTaskById(list, taskId);
      if (found) {
        task = task ? { ...task, ...found } : found;
        if (found.filename) document.title = found.filename;
        loadError = null;
        loadTimedOut = false;
        return true;
      }
    } catch (e) {
      console.warn("fetchTaskState failed:", e);
    }
    return false;
  }

  function stopPolling() {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
    if (timeoutTimer) {
      clearTimeout(timeoutTimer);
      timeoutTimer = null;
    }
  }

  function startPolling() {
    stopPolling();
    loadTimedOut = false;

    // Timeout guard: if the task is still unknown after LOAD_TIMEOUT_MS, show the error container.
    timeoutTimer = setTimeout(() => {
      if (!task) {
        loadTimedOut = true;
        stopPolling();
      }
    }, LOAD_TIMEOUT_MS);

    pollTimer = setInterval(async () => {
      if (task) {
        stopPolling();
        return;
      }
      const loaded = await fetchTaskState();
      if (loaded) {
        stopPolling();
      }
    }, POLL_INTERVAL_MS);
  }

  /**
   * User action: Retry loading / executing the task.
   * Invokes `start_task` and initiates polling again.
   */
  async function handleRetryAction() {
    if (retrying || !taskId) return;
    retrying = true;
    loadError = null;
    loadTimedOut = false;
    try {
      await safeInvoke("start_task", { id: taskId }, { suppressToast: true });
    } catch (e) {
      console.warn("start_task retry failed:", e);
      loadError = typeof e === "string" ? e : e?.message || String(e);
    } finally {
      // Re-fetch immediate state or restart polling
      const found = await fetchTaskState();
      if (!found && !task) {
        startPolling();
      }
      retrying = false;
    }
  }

  onMount(() => {
    let unmounted = false;

    // 1. Register event listeners IMMEDIATELY in onMount BEFORE awaiting async
    // settings or list calls so early progress/state events are never lost to races.
    (async () => {
      try {
        if (typeof window !== "undefined" && (window.__TAURI_INTERNALS__ || window.__TAURI__)) {
          const { listen } = await import("@tauri-apps/api/event");
          if (unmounted) return;

          unlistenProgress = await listen("sayl://progress", (event) => {
            const payload = event.payload;
            if (payload && payload.id === taskId) {
              // Never drop early progress: buffer / merge it into current task
              task = mergeProgressPayload(task, payload, taskId);
              if (task) {
                loadTimedOut = false;
                stopPolling();
              }
            }
          });

          unlistenTasksChanged = await listen("sayl://tasks-changed", (event) => {
            const payload = event.payload;
            if (!payload) return;
            const upsertList = Array.isArray(payload.upsert) ? payload.upsert : [];
            const found = upsertList.find((t) => t && t.id === taskId);
            if (found) {
              handleTaskUpdate(found);
              stopPolling();
            }
            if (Array.isArray(payload.removed) && payload.removed.includes(taskId)) {
              closeNativeWindow();
            }
          });
        }
      } catch (e) {
        console.warn("Failed to register Tauri listeners:", e);
      }
    })();

    // 2. Concurrently load settings & theme
    (async () => {
      try {
        const s = await safeInvoke("get_settings", {}, { suppressToast: true });
        if (s) {
          settings = normalizeSettings(s);
          if (s.language) setLanguage(s.language);
          applyActiveTheme(s.theme);
        } else {
          applyActiveTheme("system");
        }
      } catch {
        applyActiveTheme("system");
      }
    })();

    // 3. Immediately pull task state and show the native window
    (async () => {
      try {
        const loaded = await fetchTaskState();
        if (!loaded && !task) {
          // Poll every 200ms up to 8s
          startPolling();
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
        console.warn("Standalone window init error:", e);
        loadError = typeof e === "string" ? e : e?.message || String(e);
      }
    })();

    return () => {
      unmounted = true;
      stopPolling();
    };
  });

  onDestroy(() => {
    stopPolling();
    if (unlistenProgress) unlistenProgress();
    if (unlistenTasksChanged) unlistenTasksChanged();
  });
</script>

<main class="standalone-window-container">
  {#if viewMode === "task"}
    <DownloadWindow
      {task}
      {autoShutdown}
      autoClose={settings.close_download_window_on_finish}
      maxConnections={settings.max_connections_per_task}
      speedLimitKbps={task?.speed_limit_kbps ?? null}
      isNativeWindow={true}
      on:close={handleCloseEvent}
      on:pause={(e) => safeInvoke("pause_task", { id: e.detail })}
      on:resume={(e) => safeInvoke("start_task", { id: e.detail })}
      on:retry={async (e) => {
        await safeInvoke("start_task", { id: e.detail });
        await fetchTaskState();
      }}
      on:cancel={async (e) => {
        await safeInvoke("cancel_task", { id: e.detail });
        closeNativeWindow();
      }}
      on:reveal={(e) => runCommand("reveal_task", e.detail)}
      on:openFile={(e) => runCommand("open_task_file", e.detail)}
      on:toggleAutoShutdown={(e) => (autoShutdown = e.detail)}
      on:toggleAutoClose={async (e) => {
        settings.close_download_window_on_finish = e.detail;
        await safeInvoke("set_settings", { settings });
      }}
    />
  {:else if viewMode === "error"}
    <!-- Clean error container: never stays stuck on "..." -->
    <div class="error-container" role="alert">
      <div class="error-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24" width="40" height="40" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10" />
          <line x1="12" y1="8" x2="12" y2="12" />
          <line x1="12" y1="16" x2="12.01" y2="16" />
        </svg>
      </div>
      <h3 class="error-title">{errInfo?.title || $t("errors.unknown.title")}</h3>
      <p class="error-detail">
        <bdi dir="auto">{displayedErrorMessage}</bdi>
      </p>
      <div class="error-actions">
        <button
          type="button"
          class="btn btn-primary"
          disabled={retrying}
          on:click={handleRetryAction}
        >
          {retrying ? $t("common.saving") : $t("contextMenu.retry")}
        </button>
        <button
          type="button"
          class="btn btn-secondary"
          on:click={handleCloseEvent}
        >
          {$t("downloadWindow.close")}
        </button>
      </div>
    </div>
  {:else}
    <div class="loading-state">
      <div class="loading-spinner" aria-label={$t("task.connecting")}></div>
      <span>{$t("task.connecting")}</span>
    </div>
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

  .standalone-window-container {
    width: 100vw;
    height: 100vh;
    display: flex;
    align-items: stretch;
    justify-content: stretch;
    box-sizing: border-box;
    background: var(--color-bg);
    color: var(--color-text);
  }

  /* The native window fills its webview; the inner view drops its floating
     chrome (radius, border, shadow, fixed size) and uses the theme surface. */
  .standalone-window-container :global(.idm-window),
  .standalone-window-container :global(.download-window) {
    width: 100%;
    height: 100%;
    max-width: none;
    max-height: none;
    min-width: 0;
    min-height: 0;
    border-radius: 0;
    border: none;
    box-shadow: none;
    box-sizing: border-box;
    padding: 1rem;
    background: var(--color-bg-card);
    color: var(--color-text);
  }

  .loading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    width: 100%;
    height: 100%;
    color: var(--color-text-muted);
    font-size: 14px;
  }

  .loading-spinner {
    width: 28px;
    height: 28px;
    border: 3px solid rgba(0, 182, 189, 0.2);
    border-top-color: var(--color-accent, #00b6bd);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .error-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    width: 100%;
    height: 100%;
    padding: 2rem;
    box-sizing: border-box;
    background: var(--color-bg-card);
    color: var(--color-text);
  }

  .error-icon {
    color: var(--color-danger, #ef4444);
    margin-bottom: 1rem;
  }

  .error-title {
    margin: 0 0 0.5rem 0;
    font-size: 1.15rem;
    font-weight: 600;
  }

  .error-detail {
    margin: 0 0 1.5rem 0;
    color: var(--color-text-muted);
    font-size: 0.9rem;
    max-width: 420px;
    line-height: 1.4;
    word-break: break-word;
  }

  .error-actions {
    display: flex;
    gap: 10px;
  }

  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0.5rem 1.25rem;
    font-size: 0.9rem;
    font-weight: 500;
    border-radius: 6px;
    cursor: pointer;
    border: 1px solid transparent;
    transition: background-color 0.15s, opacity 0.15s;
    font-family: inherit;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-primary {
    background: var(--color-accent, #00b6bd);
    color: #ffffff;
  }

  .btn-primary:hover:not(:disabled) {
    background: #009aa0;
  }

  .btn-secondary {
    background: transparent;
    border-color: var(--color-border, #d1d5db);
    color: var(--color-text);
  }

  .btn-secondary:hover:not(:disabled) {
    background: var(--color-hover, rgba(0, 0, 0, 0.05));
  }
</style>

<script>
  import { onMount, onDestroy } from "svelte";
  import { t, setLanguage } from "./lib/i18n.js";
  import { safeInvoke, toasts, removeToast } from "./lib/tauri.js";
  import { canPause, canResume } from "./lib/format.js";
  import Toolbar from "./lib/Toolbar.svelte";
  import TaskList from "./lib/TaskList.svelte";
  import AddTaskDialog from "./lib/AddTaskDialog.svelte";
  import SettingsPanel from "./lib/SettingsPanel.svelte";

  let tasks = [];
  let selectedId = null;
  let addDialogOpen = false;
  let settingsOpen = false;
  let unlistenProgress = null;
  let unlistenTask = null;

  let settings = {
    download_dir: "",
    max_concurrent_tasks: 3,
    max_connections_per_task: 4,
    theme: "system",
    language: "ar",
  };

  async function loadSettings() {
    try {
      const s = await safeInvoke("get_settings");
      if (s) {
        settings = s;
        if (s.language) {
          setLanguage(s.language);
        }
        applyTheme(s.theme || "system");
      }
    } catch {
      // safeInvoke handles toast
    }
  }

  async function loadTasks() {
    try {
      const res = await safeInvoke("list_tasks");
      if (Array.isArray(res)) {
        tasks = res;
      }
    } catch {
      // safeInvoke handles toast
    }
  }

  function applyTheme(th) {
    settings.theme = th;
    if (typeof document !== "undefined") {
      if (th === "system") {
        document.documentElement.removeAttribute("data-theme");
      } else {
        document.documentElement.setAttribute("data-theme", th);
      }
    }
  }

  async function handleAddTask(url) {
    try {
      const newTask = await safeInvoke("add_task", { url });
      addDialogOpen = false;
      if (newTask) {
        tasks = [newTask, ...tasks.filter((t) => t.id !== newTask.id)];
        selectedId = newTask.id;
      }
    } catch {
      // Handled by safeInvoke toast
    }
  }

  async function handlePauseTask(id) {
    try {
      await safeInvoke("pause_task", { id });
      tasks = tasks.map((t) => (t.id === id ? { ...t, state: "Paused", speed_bps: 0, eta_secs: null } : t));
    } catch {
      // Handled by safeInvoke toast
    }
  }

  async function handleResumeTask(id) {
    try {
      await safeInvoke("start_task", { id });
      tasks = tasks.map((t) => (t.id === id ? { ...t, state: "Downloading" } : t));
    } catch {
      // Handled by safeInvoke toast
    }
  }

  async function handleCancelTask(id) {
    try {
      await safeInvoke("cancel_task", { id });
      tasks = tasks.map((t) => (t.id === id ? { ...t, state: "Cancelled", speed_bps: 0, eta_secs: null } : t));
    } catch {
      // Handled by safeInvoke toast
    }
  }

  function handleRevealTask(id) {
    safeInvoke("reveal_task", { id });
  }

  function handleResumeAll() {
    const pausable = tasks.filter((t) => canResume(t.state));
    for (const t of pausable) {
      handleResumeTask(t.id);
    }
  }

  function handlePauseAll() {
    const active = tasks.filter((t) => canPause(t.state));
    for (const t of active) {
      handlePauseTask(t.id);
    }
  }

  function handleGlobalKeydown(e) {
    const activeEl = document.activeElement;
    const isEditing = activeEl && (activeEl.tagName === "INPUT" || activeEl.tagName === "SELECT" || activeEl.tagName === "TEXTAREA");

    if ((e.ctrlKey || e.metaKey) && (e.key === "n" || e.key === "N")) {
      e.preventDefault();
      addDialogOpen = true;
      return;
    }

    if (e.key === "Escape") {
      if (addDialogOpen) {
        addDialogOpen = false;
        return;
      }
      if (settingsOpen) {
        settingsOpen = false;
        return;
      }
      selectedId = null;
      return;
    }

    if (!isEditing && selectedId) {
      const selectedTask = tasks.find((t) => t.id === selectedId);
      if (!selectedTask) return;

      if (e.code === "Space") {
        e.preventDefault();
        if (canPause(selectedTask.state)) {
          handlePauseTask(selectedId);
        } else if (canResume(selectedTask.state)) {
          handleResumeTask(selectedId);
        }
      } else if (e.key === "Delete") {
        e.preventDefault();
        handleCancelTask(selectedId);
      }
    }
  }

  onMount(async () => {
    await loadSettings();
    await loadTasks();

    // Listen to Tauri events if available
    try {
      if (typeof window !== "undefined" && (window.__TAURI_INTERNALS__ || window.__TAURI__)) {
        const { listen } = await import("@tauri-apps/api/event");

        unlistenProgress = await listen("sayl://progress", (event) => {
          const payload = event.payload;
          if (!payload || !payload.id) return;

          tasks = tasks.map((t) => {
            if (t.id === payload.id) {
              const eta = payload.speed_bps > 0 && payload.total && payload.total > payload.written
                ? Math.round((payload.total - payload.written) / payload.speed_bps)
                : null;

              return {
                ...t,
                written: payload.written,
                total_size: payload.total !== null ? payload.total : t.total_size,
                speed_bps: payload.speed_bps,
                eta_secs: eta,
                state: t.state === "Queued" || t.state === "Probing" ? "Downloading" : t.state,
              };
            }
            return t;
          });
        });

        unlistenTask = await listen("sayl://task", () => {
          loadTasks();
        });
      }
    } catch {
      // Standalone browser / test mode
    }
  });

  onDestroy(() => {
    if (unlistenProgress) unlistenProgress();
    if (unlistenTask) unlistenTask();
  });
</script>

<svelte:window on:keydown={handleGlobalKeydown} />

<div class="app-layout">
  <Toolbar
    theme={settings.theme}
    on:add={() => (addDialogOpen = true)}
    on:resumeAll={handleResumeAll}
    on:pauseAll={handlePauseAll}
    on:openFolder={() => safeInvoke("open_download_folder")}
    on:settings={() => (settingsOpen = true)}
    on:theme={(e) => applyTheme(e.detail)}
  />

  <main class="app-main">
    <TaskList
      {tasks}
      {selectedId}
      on:add={() => (addDialogOpen = true)}
      on:select={(e) => (selectedId = e.detail)}
      on:pause={(e) => handlePauseTask(e.detail)}
      on:resume={(e) => handleResumeTask(e.detail)}
      on:cancel={(e) => handleCancelTask(e.detail)}
      on:reveal={(e) => handleRevealTask(e.detail)}
    />
  </main>

  <footer class="app-footer">
    <div class="shortcuts-hint">
      <span>{$t("shortcuts.add")}</span>
      <span class="sep">•</span>
      <span>{$t("shortcuts.pauseResume")}</span>
      <span class="sep">•</span>
      <span>{$t("shortcuts.cancel")}</span>
    </div>
  </footer>

  <AddTaskDialog
    open={addDialogOpen}
    on:close={() => (addDialogOpen = false)}
    on:submit={(e) => handleAddTask(e.detail)}
  />

  <SettingsPanel
    open={settingsOpen}
    bind:settings
    on:close={() => (settingsOpen = false)}
    on:changeTheme={(e) => applyTheme(e.detail)}
  />

  {#if $toasts.length > 0}
    <div class="toast-container" role="alert" aria-live="polite">
      {#each $toasts as toast (toast.id)}
        <div class="toast-item toast-{toast.type}">
          <span class="toast-msg">{toast.message}</span>
          <button class="toast-close" on:click={() => removeToast(toast.id)} aria-label="Dismiss">
            ×
          </button>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .app-layout {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
  }

  .app-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .app-footer {
    height: 2rem;
    background-color: var(--color-bg-card);
    border-top: 1px solid var(--color-border);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 1rem;
    font-size: 0.725rem;
    color: var(--color-text-subtle);
    flex-shrink: 0;
  }

  .shortcuts-hint {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .sep {
    opacity: 0.5;
  }

  .toast-container {
    position: fixed;
    bottom: 2.75rem;
    inset-inline-end: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    z-index: 1000;
    max-width: 380px;
  }

  .toast-item {
    background-color: var(--color-bg-card);
    border-radius: var(--radius-md);
    padding: 0.75rem 1rem;
    box-shadow: var(--shadow-lg);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    font-size: 0.85rem;
    font-weight: 500;
    animation: toast-in 200ms ease;
  }

  .toast-error {
    border-inline-start: 4px solid var(--color-danger);
    color: var(--color-danger);
  }

  .toast-close {
    font-size: 1.25rem;
    line-height: 1;
    color: var(--color-text-muted);
    padding: 0 0.25rem;
  }

  .toast-close:hover {
    color: var(--color-text);
  }

  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>

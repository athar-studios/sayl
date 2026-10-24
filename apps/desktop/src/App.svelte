<script>
  import { onMount, onDestroy } from "svelte";
  import { t, currentLang, setLanguage } from "./lib/i18n.js";
  import { safeInvoke, toasts, removeToast } from "./lib/tauri.js";
  import { canPause, canResume, formatSpeed } from "./lib/format.js";
  import Toolbar from "./lib/Toolbar.svelte";
  import TaskList from "./lib/TaskList.svelte";
  import AddTaskDialog from "./lib/AddTaskDialog.svelte";
  import SettingsPanel from "./lib/SettingsPanel.svelte";

  let tasks = [];
  let selectedId = null;
  let selectedTab = "all"; // "all" | "active" | "completed" | "paused"
  let addDialogOpen = false;
  let droppedUrl = "";
  let settingsOpen = false;
  let unlistenProgress = null;
  let unlistenTask = null;
  let unlistenDragDrop = null;

  let settings = {
    download_dir: "",
    max_concurrent_tasks: 3,
    max_connections_per_task: 4,
    theme: "system",
    language: "ar",
  };

  $: activeTasksCount = tasks.filter(
    (t) => t.state === "Downloading" || t.state === "Probing" || t.state === "Queued" || t.state === "Verifying" || t.state === "Finalizing"
  ).length;

  $: completedCount = tasks.filter((t) => t.state === "Completed").length;

  $: pausedCount = tasks.filter((t) => t.state === "Paused" || t.state === "Pausing").length;

  $: totalSpeedBps = tasks.reduce(
    (sum, t) => sum + (t.state === "Downloading" && t.speed_bps ? t.speed_bps : 0),
    0
  );

  $: filteredTasks = tasks.filter((t) => {
    if (selectedTab === "active") {
      return (
        t.state === "Downloading" ||
        t.state === "Probing" ||
        t.state === "Queued" ||
        t.state === "Verifying" ||
        t.state === "Finalizing"
      );
    }
    if (selectedTab === "completed") {
      return t.state === "Completed";
    }
    if (selectedTab === "paused") {
      return t.state === "Paused" || t.state === "Pausing";
    }
    return true;
  });

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

  function handleUrlDrop(url) {
    if (!url) return;
    const trimmed = url.trim();
    if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
      droppedUrl = trimmed;
      addDialogOpen = true;
    }
  }

  function handleDomDrop(e) {
    e.preventDefault();
    const uriList = e.dataTransfer?.getData("text/uri-list");
    const plainText = e.dataTransfer?.getData("text/plain");
    const candidate = uriList || plainText;
    if (candidate) {
      handleUrlDrop(candidate);
    }
  }

  function handleDomDragOver(e) {
    e.preventDefault();
  }

  async function handleAddTask(url) {
    try {
      const newTask = await safeInvoke("add_task", { url });
      addDialogOpen = false;
      droppedUrl = "";
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
      tasks = tasks.map((t) =>
        t.id === id ? { ...t, state: "Paused", speed_bps: 0, eta_secs: null } : t
      );
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
      tasks = tasks.map((t) =>
        t.id === id ? { ...t, state: "Cancelled", speed_bps: 0, eta_secs: null } : t
      );
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
    const isEditing =
      activeEl &&
      (activeEl.tagName === "INPUT" ||
        activeEl.tagName === "SELECT" ||
        activeEl.tagName === "TEXTAREA");

    if ((e.ctrlKey || e.metaKey) && (e.key === "n" || e.key === "N")) {
      e.preventDefault();
      droppedUrl = "";
      addDialogOpen = true;
      return;
    }

    if (e.key === "Escape") {
      if (addDialogOpen) {
        addDialogOpen = false;
        droppedUrl = "";
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
              const eta =
                payload.speed_bps > 0 && payload.total && payload.total > payload.written
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

        // Tauri v2 drag-and-drop listener
        try {
          const { getCurrentWebview } = await import("@tauri-apps/api/webview");
          if (typeof getCurrentWebview === "function") {
            const webview = getCurrentWebview();
            if (webview && typeof webview.onDragDropEvent === "function") {
              unlistenDragDrop = await webview.onDragDropEvent((event) => {
                if (event.payload?.type === "drop") {
                  const paths = event.payload.paths || [];
                  for (const p of paths) {
                    if (typeof p === "string" && (p.startsWith("http://") || p.startsWith("https://"))) {
                      handleUrlDrop(p);
                      break;
                    }
                  }
                }
              });
            }
          }
        } catch (err) {
          console.warn("Tauri onDragDropEvent listener registration:", err);
        }
      }
    } catch {
      // Standalone browser / test mode
    }
  });

  onDestroy(() => {
    if (unlistenProgress) unlistenProgress();
    if (unlistenTask) unlistenTask();
    if (unlistenDragDrop) unlistenDragDrop();
  });
</script>

<svelte:window
  on:keydown={handleGlobalKeydown}
  on:dragover={handleDomDragOver}
  on:drop={handleDomDrop}
/>

<div class="app-layout">
  <Toolbar
    theme={settings.theme}
    on:add={() => {
      droppedUrl = "";
      addDialogOpen = true;
    }}
    on:resumeAll={handleResumeAll}
    on:pauseAll={handlePauseAll}
    on:openFolder={() => safeInvoke("open_download_folder")}
    on:settings={() => (settingsOpen = true)}
    on:theme={(e) => applyTheme(e.detail)}
  />

  <!-- Filter Tabs Bar (Item 6) -->
  <div class="filter-tabs-bar">
    <div class="filter-tabs" role="tablist" aria-label="Filter downloads">
      <button
        class="tab-btn"
        class:active={selectedTab === "all"}
        role="tab"
        aria-selected={selectedTab === "all"}
        on:click={() => (selectedTab = "all")}
      >
        <span>{$t("tabs.all")}</span>
        <span class="tab-badge"><bdi>{tasks.length}</bdi></span>
      </button>

      <button
        class="tab-btn"
        class:active={selectedTab === "active"}
        role="tab"
        aria-selected={selectedTab === "active"}
        on:click={() => (selectedTab = "active")}
      >
        <span>{$t("tabs.active")}</span>
        {#if activeTasksCount > 0}
          <span class="tab-badge active-badge"><bdi>{activeTasksCount}</bdi></span>
        {/if}
      </button>

      <button
        class="tab-btn"
        class:active={selectedTab === "completed"}
        role="tab"
        aria-selected={selectedTab === "completed"}
        on:click={() => (selectedTab = "completed")}
      >
        <span>{$t("tabs.completed")}</span>
        {#if completedCount > 0}
          <span class="tab-badge"><bdi>{completedCount}</bdi></span>
        {/if}
      </button>

      <button
        class="tab-btn"
        class:active={selectedTab === "paused"}
        role="tab"
        aria-selected={selectedTab === "paused"}
        on:click={() => (selectedTab = "paused")}
      >
        <span>{$t("tabs.paused")}</span>
        {#if pausedCount > 0}
          <span class="tab-badge"><bdi>{pausedCount}</bdi></span>
        {/if}
      </button>
    </div>
  </div>

  <main class="app-main">
    <TaskList
      tasks={filteredTasks}
      totalTasksCount={tasks.length}
      {selectedId}
      on:add={() => {
        droppedUrl = "";
        addDialogOpen = true;
      }}
      on:select={(e) => (selectedId = e.detail)}
      on:pause={(e) => handlePauseTask(e.detail)}
      on:resume={(e) => handleResumeTask(e.detail)}
      on:cancel={(e) => handleCancelTask(e.detail)}
      on:reveal={(e) => handleRevealTask(e.detail)}
    />
  </main>

  <!-- Summary Bar (Item 6) replaces old shortcuts bar in footer (Item 8) -->
  <footer class="app-summary-bar">
    <div class="summary-metric">
      <span class="metric-label">{$t("summary.activeTasks")}:</span>
      <strong class="metric-value"><bdi>{activeTasksCount}</bdi></strong>
    </div>

    <span class="summary-divider" aria-hidden="true">•</span>

    <div class="summary-metric">
      <span class="metric-label">{$t("summary.totalSpeed")}:</span>
      <strong class="metric-value"><bdi>{formatSpeed(totalSpeedBps, $currentLang)}</bdi></strong>
    </div>

    <span class="summary-divider" aria-hidden="true">•</span>

    <div class="summary-metric">
      <span class="metric-label">{$t("summary.freeSpace")}:</span>
      <strong class="metric-value"><bdi>—</bdi></strong>
    </div>
  </footer>

  <AddTaskDialog
    open={addDialogOpen}
    initialUrl={droppedUrl}
    on:close={() => {
      addDialogOpen = false;
      droppedUrl = "";
    }}
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
    background-color: var(--color-bg);
  }

  /* Filter Tabs Bar (Item 6) */
  .filter-tabs-bar {
    background-color: var(--color-bg-card);
    border-bottom: 1px solid var(--color-border);
    padding: 0 1.25rem;
    display: flex;
    align-items: center;
    flex-shrink: 0;
  }

  .filter-tabs {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .tab-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.65rem 0.85rem;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--color-text-muted);
    border-bottom: 2px solid transparent;
    border-radius: 0;
    transition: color var(--transition-normal), border-color var(--transition-normal);
  }

  .tab-btn:hover {
    color: var(--color-text);
  }

  .tab-btn.active {
    color: var(--color-accent);
    border-bottom-color: var(--color-accent);
  }

  .tab-badge {
    font-size: 0.7rem;
    font-weight: 700;
    padding: 0.05rem 0.4rem;
    border-radius: var(--radius-full);
    background-color: var(--color-bg-alt);
    color: var(--color-text-muted);
  }

  .tab-badge.active-badge {
    background-color: var(--color-accent-light);
    color: var(--color-accent);
  }

  .app-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* Summary Bar in Footer (Item 6) */
  .app-summary-bar {
    height: 2.25rem;
    background-color: var(--color-bg-card);
    border-top: 1px solid var(--color-border);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 1.25rem;
    gap: 1rem;
    font-size: 0.775rem;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .summary-metric {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
  }

  .metric-label {
    color: var(--color-text-subtle);
  }

  .metric-value {
    color: var(--color-text);
    font-weight: 700;
  }

  .summary-divider {
    color: var(--color-border);
  }

  .toast-container {
    position: fixed;
    bottom: 3rem;
    inset-inline-end: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    z-index: 1000;
    max-width: 380px;
  }

  .toast-item {
    background-color: var(--color-bg-card);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: 0.75rem 1rem;
    box-shadow: var(--shadow-lg);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    font-size: 0.85rem;
    font-weight: 500;
    color: var(--color-text);
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

  @media (prefers-reduced-motion: reduce) {
    .tab-btn {
      transition: none !important;
    }
    .toast-item {
      animation: none !important;
    }
  }
</style>

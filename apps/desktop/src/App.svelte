<script>
  import { onMount, onDestroy } from "svelte";
  import { t, currentLang, setLanguage } from "./lib/i18n.js";
  import { safeInvoke, toasts, removeToast, showToast } from "./lib/tauri.js";
  import { canPauseTask, canResumeTask, formatSpeed } from "./lib/format.js";
  import { DEFAULT_SETTINGS, normalizeSettings } from "./lib/settings.js";
  import { getCategoryCounts } from "./lib/classify.js";
  import {
    STATE_FILTERS,
    filterTasks,
    getStateCounts,
    matchesCategory,
  } from "./lib/filters.js";
  import {
    areAllDownloadsFinished,
    getBlockedReasonMessage,
  } from "./lib/shutdown.js";
  import { recordInvoke, startClick, endClick } from "./lib/perf.js";

  import Sidebar from "./lib/Sidebar.svelte";
  import Toolbar from "./lib/Toolbar.svelte";
  import TaskList from "./lib/TaskList.svelte";
  import AddTaskDialog from "./lib/AddTaskDialog.svelte";
  import DuplicateTaskDialog from "./lib/DuplicateTaskDialog.svelte";
  import DownloadWindow from "./lib/DownloadWindow.svelte";
  import SettingsPanel from "./lib/SettingsPanel.svelte";
  import Splash from "./lib/Splash.svelte";
  import Tour from "./lib/Tour.svelte";
  import ExtensionGuideDialog from "./lib/ExtensionGuideDialog.svelte";
  import { isTourCompleted, clearTourCompleted } from "./lib/tour.js";

  let tasks = [];
  let selectedId = null;
  let selectedCategory = "all";
  let selectedStateFilter = "all";
  let searchQuery = "";

  let addDialogOpen = false;
  let droppedUrl = "";
  let settingsOpen = false;
  let settingsInitialSection = "general";

  // Splash & Tour state
  let initialDataLoaded = false;
  let tourActive = false;
  let extensionGuideOpen = false;

  // Duplicate task dialog state
  let duplicateDialogOpen = false;
  let duplicateExistingTask = null;

  // Floating download windows: array of task IDs
  let openDownloadWindows = [];

  // Machine shutdown state (component state ONLY, NEVER persisted)
  let autoShutdown = false;
  let shutdownArmed = false;
  let shutdownSecondsLeft = null;
  let shutdownCountdownTimer = null;

  // Tauri event listener unlisten handles
  let unlistenTasksChanged = null;
  let unlistenTrayAction = null;
  let unlistenOpenSettings = null;
  let unlistenProgress = null;
  let unlistenShutdown = null;
  let unlistenDragDrop = null;

  let settings = { ...DEFAULT_SETTINGS, view_mode: "table" };

  // Instrument safeInvoke across all call sites (Req 12)
  function invoke(cmd, args) {
    recordInvoke();
    return safeInvoke(cmd, args);
  }

  // Selected task object for Toolbar enablement (Req 1)
  $: selectedTask = tasks.find((t) => t.id === selectedId) || null;
  $: canPauseAny = tasks.some((t) => canPauseTask(t.state));
  $: canResumeAny = tasks.some((t) => canResumeTask(t.state));

  // Category counts across all tasks
  $: categoryCounts = getCategoryCounts(tasks);

  // Filter tasks belonging to current category to calculate state badges (Req 2 & 3)
  $: categoryTasks = tasks.filter((t) => matchesCategory(t, selectedCategory));
  $: stateCounts = getStateCounts(categoryTasks);

  // Active / running tasks count (Req 6)
  $: activeTasksCount = tasks.filter(
    (t) =>
      t.state === "Downloading" ||
      t.state === "Probing" ||
      t.state === "Queued" ||
      t.state === "Verifying" ||
      t.state === "Finalizing"
  ).length;

  // Total transfer speed (Req 6)
  $: totalSpeedBps = tasks.reduce(
    (sum, t) => sum + (t.state === "Downloading" && t.speed_bps ? t.speed_bps : 0),
    0
  );

  // Combined filtered tasks (Req 3)
  $: filteredTasks = filterTasks(tasks, {
    category: selectedCategory,
    state: selectedStateFilter,
    query: searchQuery,
  });

  // Active floating tasks currently open
  $: activeFloatingTasks = openDownloadWindows
    .map((id) => tasks.find((t) => t.id === id))
    .filter(Boolean);

  // Watch for completion of all tasks when autoShutdown is enabled
  $: if (autoShutdown && !shutdownArmed && areAllDownloadsFinished(tasks)) {
    triggerAutoShutdown();
  }

  async function loadSettings() {
    try {
      const s = await invoke("get_settings");
      if (s) {
        settings = normalizeSettings(s);
        // Ensure default view_mode falls back to "table" if missing or invalid (Req 4)
        if (!settings.view_mode || !["cards", "table"].includes(settings.view_mode)) {
          settings.view_mode = "table";
        }
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
      const res = await invoke("list_tasks");
      if (Array.isArray(res)) {
        tasks = res;
        handleTasksUpdated(res);
      }
    } catch {
      // safeInvoke handles toast
    }
  }

  // Handle batched tasks-changed event contract (Req 11)
  function handleTasksChanged(payload) {
    if (!payload || typeof payload !== "object") return;

    if (payload.full) {
      tasks = Array.isArray(payload.upsert) ? payload.upsert : [];
    } else {
      let next = [...tasks];

      if (Array.isArray(payload.removed) && payload.removed.length > 0) {
        const removedSet = new Set(payload.removed);
        next = next.filter((t) => !removedSet.has(t.id));
      }

      if (Array.isArray(payload.upsert) && payload.upsert.length > 0) {
        const upsertMap = new Map(payload.upsert.map((t) => [t.id, t]));
        next = next.map((t) => {
          if (upsertMap.has(t.id)) {
            const updated = { ...t, ...upsertMap.get(t.id) };
            upsertMap.delete(t.id);
            return updated;
          }
          return t;
        });
        for (const newTask of upsertMap.values()) {
          next.push(newTask);
        }
      }

      tasks = next;
    }

    handleTasksUpdated(tasks);
  }

  function handleTasksUpdated(newTasks) {
    if (settings.close_download_window_on_finish) {
      for (const t of newTasks) {
        if (t.state === "Completed" && openDownloadWindows.includes(t.id)) {
          openDownloadWindows = openDownloadWindows.filter((id) => id !== t.id);
        }
      }
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

  async function handleAddTask(url, force = false) {
    const start = startClick();
    try {
      const outcome = await invoke("add_task", { url, force });
      addDialogOpen = false;
      droppedUrl = "";

      if (!outcome) return;

      if (outcome.kind === "duplicate") {
        duplicateExistingTask = outcome.existing;
        duplicateDialogOpen = true;
        return;
      }

      if (outcome.kind === "added") {
        const newTask = outcome.task;
        tasks = [newTask, ...tasks.filter((t) => t.id !== newTask.id)];
        selectedId = newTask.id;

        if (settings.show_download_window) {
          if (!openDownloadWindows.includes(newTask.id)) {
            openDownloadWindows = [...openDownloadWindows, newTask.id];
          }
        }
      }
    } catch {
      // Handled by safeInvoke toast
    } finally {
      endClick(start, "add-task");
    }
  }

  function handleOpenExistingTask(id) {
    duplicateDialogOpen = false;
    duplicateExistingTask = null;
    selectedId = id;
  }

  function handleDownloadAgain(url) {
    duplicateDialogOpen = false;
    duplicateExistingTask = null;
    handleAddTask(url, true);
  }

  function handleCancelDuplicateDialog() {
    duplicateDialogOpen = false;
    duplicateExistingTask = null;
  }

  async function handlePauseTask(id) {
    const start = startClick();
    try {
      await invoke("pause_task", { id });
      tasks = tasks.map((t) =>
        t.id === id ? { ...t, state: "Paused", speed_bps: 0, eta_secs: null } : t
      );
    } catch {
      // Handled by safeInvoke toast
    } finally {
      endClick(start, "pause-task");
    }
  }

  async function handleResumeTask(id) {
    const start = startClick();
    try {
      await invoke("start_task", { id });
      tasks = tasks.map((t) => (t.id === id ? { ...t, state: "Downloading" } : t));
    } catch {
      // Handled by safeInvoke toast
    } finally {
      endClick(start, "resume-task");
    }
  }

  async function handleRetryTask(id) {
    const start = startClick();
    try {
      await invoke("start_task", { id });
      tasks = tasks.map((t) => (t.id === id ? { ...t, state: "Downloading" } : t));
    } catch {
      // Handled by safeInvoke toast
    } finally {
      endClick(start, "retry-task");
    }
  }

  async function handleCancelTask(id) {
    const start = startClick();
    try {
      await invoke("cancel_task", { id });
      tasks = tasks.map((t) =>
        t.id === id ? { ...t, state: "Cancelled", speed_bps: 0, eta_secs: null } : t
      );
    } catch {
      // Handled by safeInvoke toast
    } finally {
      endClick(start, "cancel-task");
    }
  }

  function handleRevealTask(id) {
    const start = startClick();
    invoke("reveal_task", { id });
    endClick(start, "reveal-task");
  }

  function handleOpenFileTask(id) {
    const start = startClick();
    invoke("open_task_file", { id });
    endClick(start, "open-task-file");
  }

  function handleResumeAll() {
    const start = startClick();
    const resumable = tasks.filter((t) => canResumeTask(t.state));
    for (const t of resumable) {
      handleResumeTask(t.id);
    }
    endClick(start, "resume-all");
  }

  function handlePauseAll() {
    const start = startClick();
    const active = tasks.filter((t) => canPauseTask(t.state));
    for (const t of active) {
      handlePauseTask(t.id);
    }
    endClick(start, "pause-all");
  }

  async function handleToggleSidebar(collapsed) {
    settings.sidebar_collapsed = collapsed;
    try {
      const full = normalizeSettings(settings);
      await invoke("set_settings", { settings: full });
    } catch {
      //
    }
  }

  async function handleSetViewMode(mode) {
    settings.view_mode = mode;
    try {
      const full = normalizeSettings(settings);
      await invoke("set_settings", { settings: full });
    } catch {
      //
    }
  }

  async function handleToggleAutoClose(val) {
    settings.close_download_window_on_finish = val;
    try {
      const full = normalizeSettings(settings);
      await invoke("set_settings", { settings: full });
    } catch {
      //
    }
  }

  function handleCloseDownloadWindow(id) {
    openDownloadWindows = openDownloadWindows.filter((wId) => wId !== id);
  }

  async function triggerAutoShutdown() {
    try {
      const res = await invoke("arm_shutdown", { seconds: 60 });
      if (res) {
        shutdownArmed = res.armed;
        shutdownSecondsLeft = res.seconds_left ?? 60;
        startCountdownTimer();
        if (res.blocked_reason) {
          const msg = getBlockedReasonMessage(res.blocked_reason, $currentLang);
          showToast(msg, "error");
          autoShutdown = false;
        }
      }
    } catch {
      autoShutdown = false;
    }
  }

  async function handleCancelShutdown() {
    try {
      await invoke("cancel_shutdown");
    } catch {
      //
    }
    autoShutdown = false;
    shutdownArmed = false;
    shutdownSecondsLeft = null;
    stopCountdownTimer();
  }

  function startCountdownTimer() {
    stopCountdownTimer();
    shutdownCountdownTimer = setInterval(() => {
      if (shutdownSecondsLeft !== null && shutdownSecondsLeft > 0) {
        shutdownSecondsLeft -= 1;
      }
    }, 1000);
  }

  function stopCountdownTimer() {
    if (shutdownCountdownTimer) {
      clearInterval(shutdownCountdownTimer);
      shutdownCountdownTimer = null;
    }
  }

  function handleGlobalKeydown(e) {
    if (tourActive || extensionGuideOpen) {
      return;
    }

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
      if (duplicateDialogOpen) {
        handleCancelDuplicateDialog();
        return;
      }
      if (addDialogOpen) {
        addDialogOpen = false;
        droppedUrl = "";
        return;
      }
      if (settingsOpen) {
        settingsOpen = false;
        return;
      }
      if (shutdownArmed) {
        handleCancelShutdown();
        return;
      }
      selectedId = null;
      return;
    }

    if (!isEditing && selectedId) {
      const selectedT = tasks.find((t) => t.id === selectedId);
      if (!selectedT) return;

      if (e.code === "Space") {
        e.preventDefault();
        if (canPauseTask(selectedT.state)) {
          handlePauseTask(selectedId);
        } else if (canResumeTask(selectedT.state)) {
          handleResumeTask(selectedId);
        }
      } else if (e.key === "Delete") {
        e.preventDefault();
        handleCancelTask(selectedId);
      }
    }
  }

  function handleReplayTour() {
    clearTourCompleted();
    settingsOpen = false;
    setTimeout(() => {
      tourActive = true;
    }, 200);
  }

  function handleOpenExtensionGuideFromTour() {
    tourActive = false;
    extensionGuideOpen = true;
  }

  onMount(async () => {
    // Initial mount: exactly one get_settings + one list_tasks (Req 11)
    await loadSettings();
    await loadTasks();
    initialDataLoaded = true;

    // First-run onboarding tour (runs once per machine)
    if (!isTourCompleted()) {
      setTimeout(() => {
        tourActive = true;
      }, 400);
    }

    // Listen to Tauri events if available
    try {
      if (typeof window !== "undefined" && (window.__TAURI_INTERNALS__ || window.__TAURI__)) {
        const { listen } = await import("@tauri-apps/api/event");

        // Batched tasks-changed contract (Req 11)
        unlistenTasksChanged = await listen("sayl://tasks-changed", (event) => {
          handleTasksChanged(event.payload);
        });

        // Tray action listener (Req 11)
        unlistenTrayAction = await listen("sayl://tray-action", (event) => {
          const action = event.payload;
          if (action === "add") {
            droppedUrl = "";
            addDialogOpen = true;
          } else if (action === "pause-all") {
            handlePauseAll();
          } else if (action === "resume-all") {
            handleResumeAll();
          }
        });

        // Open settings listener (Req 11)
        unlistenOpenSettings = await listen("sayl://open-settings", () => {
          settingsOpen = true;
        });

        // Backward-compatible progress listener (Req 11)
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

        // Shutdown countdown listener
        unlistenShutdown = await listen("sayl://shutdown", (event) => {
          const payload = event.payload;
          if (!payload) return;

          if (payload.armed) {
            shutdownArmed = true;
            shutdownSecondsLeft = payload.seconds_left ?? 60;
            startCountdownTimer();
          } else {
            shutdownArmed = false;
            shutdownSecondsLeft = null;
            stopCountdownTimer();
            if (payload.blocked_reason) {
              const msg = getBlockedReasonMessage(payload.blocked_reason, $currentLang);
              showToast(msg, "error");
              autoShutdown = false;
            }
          }
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
    if (unlistenTasksChanged) unlistenTasksChanged();
    if (unlistenTrayAction) unlistenTrayAction();
    if (unlistenOpenSettings) unlistenOpenSettings();
    if (unlistenProgress) unlistenProgress();
    if (unlistenDragDrop) unlistenDragDrop();
    if (unlistenShutdown) unlistenShutdown();
    stopCountdownTimer();
  });
</script>

<svelte:window
  on:keydown={handleGlobalKeydown}
  on:dragover={handleDomDragOver}
  on:drop={handleDomDrop}
/>

<!-- Splash Screen (Req 1) -->
<Splash ready={initialDataLoaded} />

<div class="app-layout">
  <!-- Collapsible Category & Status Sidebar (Req 2) -->
  <Sidebar
    collapsed={settings.sidebar_collapsed}
    {selectedCategory}
    {selectedStateFilter}
    {categoryCounts}
    statusCounts={stateCounts}
    on:select={(e) => (selectedCategory = e.detail)}
    on:selectState={(e) => (selectedStateFilter = e.detail)}
    on:toggleCollapse={(e) => handleToggleSidebar(e.detail)}
  />

  <div class="app-content">
    <!-- Top Toolbar (Req 1) -->
    <Toolbar
      {selectedTask}
      {canPauseAny}
      {canResumeAny}
      theme={settings.theme}
      {autoShutdown}
      on:add={() => {
        droppedUrl = "";
        addDialogOpen = true;
      }}
      on:resume={() => selectedId && handleResumeTask(selectedId)}
      on:pause={() => selectedId && handlePauseTask(selectedId)}
      on:pauseAll={handlePauseAll}
      on:delete={() => selectedId && handleCancelTask(selectedId)}
      on:retry={() => selectedId && handleRetryTask(selectedId)}
      on:openFolder={() => invoke("open_download_folder")}
      on:settings={() => {
        settingsInitialSection = "general";
        settingsOpen = true;
      }}
      on:theme={(e) => applyTheme(e.detail)}
      on:toggleAutoShutdown={(e) => (autoShutdown = e.detail)}
    />

    <!-- Filter & Search Bar: State filter tabs, Search box, View mode toggle (Req 3) -->
    <div id="tour-search-filter" class="filter-controls-bar">
      <div class="state-filter-tabs" role="tablist" aria-label="Filter downloads by state">
        {#each STATE_FILTERS as sf (sf.id)}
          {@const count = stateCounts[sf.id] || 0}
          <button
            class="filter-tab-btn"
            class:active={selectedStateFilter === sf.id}
            role="tab"
            aria-selected={selectedStateFilter === sf.id}
            on:click={() => (selectedStateFilter = sf.id)}
          >
            <span>{$t("filter." + sf.id)}</span>
            <span class="tab-badge" class:has-items={count > 0}><bdi>{count}</bdi></span>
          </button>
        {/each}
      </div>

      <div class="search-and-view-controls">
        <div class="search-box">
          <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" class="search-icon" aria-hidden="true">
            <path
              fill-rule="evenodd"
              d="M8 4a4 4 0 100 8 4 4 0 000-8zM2 8a6 6 0 1110.89 3.476l4.817 4.817a1 1 0 01-1.414 1.414l-4.816-4.816A6 6 0 012 8z"
              clip-rule="evenodd"
            />
          </svg>
          <input
            type="text"
            class="search-input"
            placeholder={$t("filter.searchPlaceholder")}
            bind:value={searchQuery}
          />
          {#if searchQuery}
            <button class="search-clear-btn" on:click={() => (searchQuery = "")} aria-label={$t("filter.clearSearch")}>
              ×
            </button>
          {/if}
        </div>

        <div class="view-mode-toggle" role="radiogroup" aria-label={$t("settings.viewMode")}>
          <button
            class="view-mode-btn"
            class:active={settings.view_mode === 'table'}
            title={$t("view.table")}
            aria-label={$t("view.table")}
            aria-checked={settings.view_mode === 'table'}
            role="radio"
            on:click={() => handleSetViewMode("table")}
          >
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
              <path
                fill-rule="evenodd"
                d="M3 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm0 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm0 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm0 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1z"
                clip-rule="evenodd"
              />
            </svg>
          </button>
          <button
            class="view-mode-btn"
            class:active={settings.view_mode === 'cards'}
            title={$t("view.cards")}
            aria-label={$t("view.cards")}
            aria-checked={settings.view_mode === 'cards'}
            role="radio"
            on:click={() => handleSetViewMode("cards")}
          >
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
              <path d="M5 3a2 2 0 00-2 2v2a2 2 0 002 2h2a2 2 0 002-2V5a2 2 0 00-2-2H5zM5 11a2 2 0 00-2 2v2a2 2 0 002 2h2a2 2 0 002-2v-2a2 2 0 00-2-2H5zM11 5a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V5zM11 13a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z" />
            </svg>
          </button>
        </div>
      </div>
    </div>

    <!-- Machine Shutdown Countdown Banner -->
    {#if shutdownArmed && shutdownSecondsLeft !== null}
      <div class="shutdown-countdown-banner" role="alert" aria-live="assertive">
        <div class="countdown-content">
          <div class="countdown-icon" aria-hidden="true">
            <svg viewBox="0 0 20 20" fill="currentColor" width="22" height="22">
              <path
                fill-rule="evenodd"
                d="M10 2a1 1 0 011 1v7a1 1 0 11-2 0V3a1 1 0 011-1zm4.95 3.05a1 1 0 011.414 1.414A7.97 7.97 0 0118 10a8 8 0 11-8-8c.7 0 1.378.09 2.02.26a1 1 0 11-.518 1.932A5.98 5.98 0 0010 4a6 6 0 106 6c0-1.2-.352-2.317-.96-3.26a1 1 0 01-.09-1.69z"
                clip-rule="evenodd"
              />
            </svg>
          </div>
          <span class="countdown-text">
            {$t("shutdown.countdownTitle")}
            <strong class="countdown-num"><bdi>{shutdownSecondsLeft}</bdi></strong>
            {$t("shutdown.secondsSuffix")}
          </span>
        </div>
        <button class="btn btn-cancel-shutdown" on:click={handleCancelShutdown}>
          {$t("shutdown.cancelShutdown")}
        </button>
      </div>
    {/if}

    <!-- Main Tasks View (Default Table or Cards) (Req 4) -->
    <main id="tour-task-area" class="app-main">
      <TaskList
        tasks={filteredTasks}
        totalTasksCount={tasks.length}
        {selectedId}
        viewMode={settings.view_mode || "table"}
        on:add={() => {
          droppedUrl = "";
          addDialogOpen = true;
        }}
        on:select={(e) => (selectedId = e.detail)}
        on:pause={(e) => handlePauseTask(e.detail)}
        on:resume={(e) => handleResumeTask(e.detail)}
        on:retry={(e) => handleRetryTask(e.detail)}
        on:cancel={(e) => handleCancelTask(e.detail)}
        on:reveal={(e) => handleRevealTask(e.detail)}
        on:openFile={(e) => handleOpenFileTask(e.detail)}
      />
    </main>

    <!-- Bottom Status Bar (Req 6) -->
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

      {#if settings.prevent_sleep && activeTasksCount > 0}
        <span class="summary-divider" aria-hidden="true">•</span>
        <div class="summary-indicator indicator-prevent-sleep" title={$t("summary.preventSleep")}>
          <span class="indicator-dot"></span>
          <span class="indicator-label">{$t("summary.preventSleep")}</span>
        </div>
      {/if}

      {#if autoShutdown}
        <span class="summary-divider" aria-hidden="true">•</span>
        <div class="summary-indicator indicator-shutdown" class:armed={shutdownArmed}>
          <span class="indicator-dot"></span>
          <span class="indicator-label">
            {shutdownArmed ? $t("summary.shutdownArmed") : $t("summary.autoShutdown")}
          </span>
        </div>
      {/if}
    </footer>
  </div>

  <!-- Add Task Dialog -->
  <AddTaskDialog
    open={addDialogOpen}
    initialUrl={droppedUrl}
    on:close={() => {
      addDialogOpen = false;
      droppedUrl = "";
    }}
    on:submit={(e) => handleAddTask(e.detail)}
  />

  <!-- Duplicate Task Dialog -->
  <DuplicateTaskDialog
    open={duplicateDialogOpen}
    existingTask={duplicateExistingTask}
    on:openExisting={(e) => handleOpenExistingTask(e.detail)}
    on:downloadAgain={(e) => handleDownloadAgain(e.detail)}
    on:cancel={handleCancelDuplicateDialog}
  />

  <!-- Settings Panel -->
  <SettingsPanel
    open={settingsOpen}
    initialSection={settingsInitialSection}
    bind:settings
    on:close={() => {
      settingsOpen = false;
      settingsInitialSection = "general";
    }}
    on:changeTheme={(e) => applyTheme(e.detail)}
    on:replayTour={handleReplayTour}
  />

  <!-- First-Run Onboarding Tour (Req 2) -->
  <Tour
    active={tourActive}
    on:finish={() => (tourActive = false)}
    on:skip={() => (tourActive = false)}
    on:openExtensionGuide={handleOpenExtensionGuideFromTour}
  />

  <!-- Extension Guide Dialog (Req 4 / Tour Step 6) -->
  <ExtensionGuideDialog
    open={extensionGuideOpen}
    on:close={() => (extensionGuideOpen = false)}
  />

  <!-- Floating Download Windows Stack (Req 8) -->
  {#if activeFloatingTasks.length > 0}
    <div class="floating-windows-stack" role="region" aria-label={$t("downloadWindow.title")}>
      {#each activeFloatingTasks as floatingTask (floatingTask.id)}
        <DownloadWindow
          task={floatingTask}
          {autoShutdown}
          autoClose={settings.close_download_window_on_finish}
          on:close={(e) => handleCloseDownloadWindow(e.detail)}
          on:pause={(e) => handlePauseTask(e.detail)}
          on:resume={(e) => handleResumeTask(e.detail)}
          on:retry={(e) => handleRetryTask(e.detail)}
          on:cancel={(e) => handleCancelTask(e.detail)}
          on:reveal={(e) => handleRevealTask(e.detail)}
          on:openFile={(e) => handleOpenFileTask(e.detail)}
          on:toggleAutoShutdown={(e) => (autoShutdown = e.detail)}
          on:toggleAutoClose={(e) => handleToggleAutoClose(e.detail)}
        />
      {/each}
    </div>
  {/if}

  <!-- Toast Notification Bar -->
  {#if $toasts.length > 0}
    <div class="toast-container" role="alert" aria-live="polite">
      {#each $toasts as toast (toast.id)}
        <div class="toast-item" class:toast-error={toast.kind === 'error'}>
          <span class="toast-text">{toast.message}</span>
          <button
            class="toast-close"
            on:click={() => removeToast(toast.id)}
            aria-label="Close notification"
          >
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
    height: 100vh;
    width: 100vw;
    overflow: hidden;
    background-color: var(--color-bg);
  }

  .app-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    overflow: hidden;
  }

  .filter-controls-bar {
    background-color: var(--color-bg-card);
    border-bottom: 1px solid var(--color-border);
    padding: 0.35rem 1.25rem;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
    flex-shrink: 0;
  }

  .state-filter-tabs {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  .filter-tab-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.5rem 0.75rem;
    font-size: 0.825rem;
    font-weight: 600;
    color: var(--color-text-muted);
    border-bottom: 2px solid transparent;
    border-radius: 0;
    white-space: nowrap;
    transition: color var(--transition-normal), border-color var(--transition-normal);
    cursor: pointer;
  }

  .filter-tab-btn:hover {
    color: var(--color-text);
  }

  .filter-tab-btn.active {
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

  .filter-tab-btn.active .tab-badge.has-items {
    background-color: var(--color-accent-light);
    color: var(--color-accent);
  }

  .search-and-view-controls {
    display: flex;
    align-items: center;
    gap: 0.65rem;
  }

  .search-box {
    position: relative;
    display: flex;
    align-items: center;
    min-width: 220px;
    max-width: 320px;
  }

  .search-icon {
    position: absolute;
    inset-inline-start: 0.65rem;
    color: var(--color-text-subtle);
    pointer-events: none;
  }

  .search-input {
    width: 100%;
    padding: 0.35rem 1.85rem 0.35rem 2rem;
    font-size: 0.8rem;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background-color: var(--color-bg-alt);
    color: var(--color-text);
    transition: border-color var(--transition-normal), background-color var(--transition-normal);
  }

  .search-input:focus {
    outline: none;
    border-color: var(--color-border-focus);
    background-color: var(--color-bg-card);
  }

  .search-clear-btn {
    position: absolute;
    inset-inline-end: 0.45rem;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 1.1rem;
    line-height: 1;
    padding: 0.1rem 0.25rem;
    border-radius: 4px;
    cursor: pointer;
  }

  .search-clear-btn:hover {
    color: var(--color-text);
  }

  .view-mode-toggle {
    display: flex;
    align-items: center;
    background-color: var(--color-bg-alt);
    padding: 0.15rem;
    border-radius: var(--radius-sm);
    gap: 0.15rem;
  }

  .view-mode-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.85rem;
    height: 1.85rem;
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
    background: transparent;
    transition: background-color var(--transition-normal), color var(--transition-normal);
    cursor: pointer;
  }

  .view-mode-btn:hover {
    color: var(--color-text);
  }

  .view-mode-btn.active {
    background-color: var(--color-bg-card);
    color: var(--color-accent);
    box-shadow: var(--shadow-sm);
  }

  /* Machine Shutdown Countdown Banner */
  .shutdown-countdown-banner {
    background: linear-gradient(135deg, rgba(220, 38, 38, 0.15), rgba(11, 26, 78, 0.25));
    border-bottom: 2px solid var(--color-danger);
    padding: 0.65rem 1.25rem;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    animation: banner-in 200ms ease;
  }

  .countdown-content {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    color: var(--color-danger);
    font-size: 0.9rem;
  }

  .countdown-num {
    font-size: 1.15rem;
    font-weight: 800;
  }

  .btn-cancel-shutdown {
    background-color: var(--color-danger);
    color: #ffffff;
    padding: 0.4rem 1rem;
    font-size: 0.825rem;
    font-weight: 700;
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-sm);
    transition: filter var(--transition-normal);
    cursor: pointer;
  }

  .btn-cancel-shutdown:hover {
    filter: brightness(1.1);
  }

  .app-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* Bottom Summary Bar (Req 6) */
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

  /* Status Bar Indicators */
  .summary-indicator {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.75rem;
    font-weight: 600;
  }

  .indicator-prevent-sleep {
    color: var(--state-running, #00b6bd);
  }

  .indicator-shutdown {
    color: var(--state-paused, #d97706);
  }

  .indicator-shutdown.armed {
    color: var(--state-failed, #dc2626);
    animation: pulse-armed 1.5s infinite;
  }

  .indicator-dot {
    width: 0.45rem;
    height: 0.45rem;
    border-radius: 50%;
    background-color: currentColor;
  }

  @keyframes pulse-armed {
    0%, 100% {
      opacity: 1;
    }
    50% {
      opacity: 0.4;
    }
  }

  /* Floating Download Windows Stack */
  .floating-windows-stack {
    position: fixed;
    bottom: 3rem;
    inset-inline-end: 1.5rem;
    display: flex;
    flex-direction: column-reverse;
    gap: 0.75rem;
    z-index: 800;
    max-height: calc(100vh - 5rem);
    overflow-y: auto;
    pointer-events: none;
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
    cursor: pointer;
  }

  .toast-close:hover {
    color: var(--color-text);
  }

  @keyframes banner-in {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
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
    .filter-tab-btn,
    .view-mode-btn {
      transition: none !important;
    }
    .toast-item,
    .shutdown-countdown-banner {
      animation: none !important;
    }
    .indicator-shutdown.armed {
      animation: none !important;
    }
  }
</style>

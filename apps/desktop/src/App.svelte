<script>
  import { onMount, onDestroy } from "svelte";
  import { t, currentLang, setLanguage, toggleLanguage } from "./lib/i18n.js";
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

  import MenuBar from "./lib/MenuBar.svelte";
  import Sidebar from "./lib/Sidebar.svelte";
  import Toolbar from "./lib/Toolbar.svelte";
  import TaskList from "./lib/TaskList.svelte";
  import AddTaskDialog from "./lib/AddTaskDialog.svelte";
  import DuplicateTaskDialog from "./lib/DuplicateTaskDialog.svelte";
  import DeleteConfirmDialog from "./lib/DeleteConfirmDialog.svelte";
  import DownloadWindow from "./lib/DownloadWindow.svelte";
  import SettingsPanel from "./lib/SettingsPanel.svelte";
  import Splash from "./lib/Splash.svelte";
  import Tour from "./lib/Tour.svelte";
  import ExtensionGuideDialog from "./lib/ExtensionGuideDialog.svelte";
  import SchedulerDialog from "./lib/SchedulerDialog.svelte";
  import iconSvg from "./assets/icon.svg";
  import logoFull from "./assets/logo-full.svg";
  import logoWhite from "./assets/logo-white.svg";
  import { isTourCompleted, clearTourCompleted } from "./lib/tour.js";
  import {
    handleItemClick,
    selectAll,
    clearSelection,
    moveSelection,
  } from "./lib/selection.js";

  let tasks = [];
  let selectedId = null;
  let selectedIds = [];
  let lastAnchorId = null;
  let selectedCategory = "all";
  let selectedStateFilter = "all";
  let searchQuery = "";

  let addDialogOpen = false;
  let droppedUrl = "";
  let settingsOpen = false;
  let settingsInitialSection = "general";
  let schedulerOpen = false;
  let showToolbar = true;
  let showStatusbar = true;

  // Splash & Tour state
  let initialDataLoaded = false;
  let tourActive = false;
  let extensionGuideOpen = false;

  // Duplicate task dialog state
  let duplicateDialogOpen = false;
  let duplicateExistingTask = null;

  // Delete confirmation dialog state
  let deleteDialogOpen = false;
  let deleteTaskIds = [];

  // Floating download windows: array of task IDs
  let openDownloadWindows = [];
  const openWindowsSet = new Set();

  // Machine shutdown state (component state ONLY, NEVER persisted)
  let autoShutdown = false;
  let shutdownArmed = false;
  let shutdownSecondsLeft = null;
  let shutdownCountdownTimer = null;

  // Tauri event listener unlisten handles
  let unlistenTasksChanged = null;
  let unlistenTrayAction = null;
  let unlistenOpenSettings = null;
  let unlistenWindowClosed = null;
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
  $: selectedTask = tasks.find((t) => t.id === selectedId) || (selectedIds.length === 1 ? tasks.find((t) => t.id === selectedIds[0]) : null) || null;
  $: tasksToDelete = tasks.filter((t) => deleteTaskIds.includes(t.id));
  $: deleteDialogFilename = tasksToDelete.length === 1 ? tasksToDelete[0].filename : "";
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
    .filter((t) => t && t.state !== "Cancelled");

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
    for (const t of newTasks) {
      if (
        (t.state === "Completed" && settings.close_download_window_on_finish) ||
        t.state === "Cancelled"
      ) {
        openWindowsSet.delete(t.id);
        openDownloadWindows = openDownloadWindows.filter((id) => id !== t.id);
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

        if (settings.show_download_window && newTask.state !== "Cancelled") {
          handleOpenDownloadWindow(newTask.id);
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

  function handleSelectTask(detail) {
    if (!detail) return;
    if (typeof detail === "string") {
      selectedIds = [detail];
      selectedId = detail;
      lastAnchorId = detail;
    } else if (typeof detail === "object") {
      const { id, ctrlKey, shiftKey } = detail;
      const res = handleItemClick({
        items: filteredTasks,
        selectedIds,
        clickedId: id,
        event: { ctrlKey, shiftKey },
        lastAnchorId,
      });
      selectedIds = res.selectedIds;
      lastAnchorId = res.lastAnchorId;
      selectedId = selectedIds[selectedIds.length - 1] || null;
    }
  }

  function handleTriggerDelete(targetId) {
    let idsToDelete = [];
    if (typeof targetId === "string" && targetId) {
      if (selectedIds.includes(targetId)) {
        idsToDelete = [...selectedIds];
      } else {
        idsToDelete = [targetId];
      }
    } else if (selectedIds.length > 0) {
      idsToDelete = [...selectedIds];
    } else if (selectedId) {
      idsToDelete = [selectedId];
    }

    if (idsToDelete.length === 0) return;

    deleteTaskIds = idsToDelete;
    deleteDialogOpen = true;
  }

  async function handleConfirmDelete(e) {
    const { deleteFiles } = e.detail || { deleteFiles: false };
    const ids = [...deleteTaskIds];
    deleteDialogOpen = false;
    deleteTaskIds = [];

    const start = startClick();
    try {
      await invoke("delete_tasks", { ids, delete_files: deleteFiles });
      const idSet = new Set(ids);
      tasks = tasks.filter((t) => !idSet.has(t.id));
      selectedIds = selectedIds.filter((id) => !idSet.has(id));
      selectedId = selectedIds[selectedIds.length - 1] || null;
    } catch {
      // safeInvoke handles toast
    } finally {
      endClick(start, "delete-tasks");
    }
  }

  function handleCancelDeleteDialog() {
    deleteDialogOpen = false;
    deleteTaskIds = [];
  }

  async function handleCancelTask(id) {
    const start = startClick();
    try {
      await invoke("cancel_task", { id });
      tasks = tasks.map((t) =>
        t.id === id ? { ...t, state: "Cancelled", speed_bps: 0, eta_secs: null } : t
      );
      openDownloadWindows = openDownloadWindows.filter((wId) => wId !== id);
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

  function handleDeleteCompleted() {
    const completedTasks = tasks.filter((t) => t.state === "Completed");
    if (completedTasks.length === 0) return;
    deleteTaskIds = completedTasks.map((t) => t.id);
    deleteDialogOpen = true;
  }

  function handleSelectAll() {
    selectedIds = selectAll(filteredTasks.length > 0 ? filteredTasks : tasks);
    if (selectedIds.length > 0) {
      selectedId = selectedIds[0];
    }
  }

  async function handleOpenDownloadWindow(taskId) {
    if (!taskId) return;
    if (openWindowsSet.has(taskId)) {
      if (typeof window !== "undefined" && (window.__TAURI_INTERNALS__ || window.__TAURI__)) {
        try {
          const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
          const existing = await WebviewWindow.getByLabel(`download-${taskId}`);
          if (existing) {
            await existing.show();
            await existing.setFocus();
          }
        } catch {}
      }
      return;
    }

    if (typeof window !== "undefined" && (window.__TAURI_INTERNALS__ || window.__TAURI__)) {
      try {
        const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
        const existing = await WebviewWindow.getByLabel(`download-${taskId}`);
        if (existing) {
          openWindowsSet.add(taskId);
          await existing.show();
          await existing.setFocus();
          return;
        }
      } catch {}

      openWindowsSet.add(taskId);
      try {
        await invoke("open_download_window", { id: taskId });
        return;
      } catch (err) {
        console.warn("Failed to open native download window:", err);
        openWindowsSet.delete(taskId);
        return;
      }
    }

    // Fallback for browser mock/preview:
    openWindowsSet.add(taskId);
    if (!openDownloadWindows.includes(taskId)) {
      openDownloadWindows = [...openDownloadWindows, taskId];
    }
  }

  function handleOpenProperties(id) {
    const targetId = id || selectedId || (selectedIds.length > 0 ? selectedIds[0] : null);
    if (targetId) {
      handleOpenDownloadWindow(targetId);
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
    openWindowsSet.delete(id);
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
      if (deleteDialogOpen) {
        handleCancelDeleteDialog();
        return;
      }
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
      selectedIds = [];
      lastAnchorId = null;
      return;
    }

    if (!isEditing) {
      if ((e.ctrlKey || e.metaKey) && (e.key === "a" || e.key === "A")) {
        e.preventDefault();
        selectedIds = selectAll(filteredTasks);
        selectedId = selectedIds[selectedIds.length - 1] || null;
        lastAnchorId = selectedIds[0] || null;
        return;
      }

      if (e.key === "Delete") {
        if (selectedIds.length > 0 || selectedId) {
          e.preventDefault();
          handleTriggerDelete();
          return;
        }
      }

      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        if (filteredTasks.length > 0) {
          e.preventDefault();
          const direction = e.key === "ArrowDown" ? "down" : "up";
          const res = moveSelection({
            items: filteredTasks,
            selectedIds,
            direction,
            shiftKey: e.shiftKey,
            lastAnchorId,
          });
          selectedIds = res.selectedIds;
          lastAnchorId = res.lastAnchorId;
          selectedId = res.activeId;
          return;
        }
      }

      if (selectedId) {
        const selectedT = tasks.find((t) => t.id === selectedId);
        if (selectedT && e.code === "Space") {
          e.preventDefault();
          if (canPauseTask(selectedT.state)) {
            handlePauseTask(selectedId);
          } else if (canResumeTask(selectedT.state)) {
            handleResumeTask(selectedId);
          }
        }
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

    // Tour is available on demand via Help -> Tour (no auto-pop on startup to avoid blocking UI)

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

        // Window closed listener for deduplication tracking
        unlistenWindowClosed = await listen("sayl://window-closed", (event) => {
          const id = event.payload;
          if (id) {
            openWindowsSet.delete(id);
            openDownloadWindows = openDownloadWindows.filter((wId) => wId !== id);
          }
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
    if (unlistenWindowClosed) unlistenWindowClosed();
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

<div class="app-layout" role="application">
  <div class="app-content">
    <!-- App Brand Header / Titlebar -->
    <header class="app-titlebar">
      <div class="titlebar-brand">
        <img src={iconSvg} alt="SAYL" class="titlebar-logo" />
        <span class="titlebar-title">{$t("app.fullName")}</span>
        <span class="titlebar-version">v0.1.0</span>
      </div>
    </header>

    <!-- Classic Menu Bar (Requirement 3) -->
    <MenuBar
      viewMode={settings.view_mode || "table"}
      showSidebar={!settings.sidebar_collapsed}
      {showToolbar}
      {showStatusbar}
      canResumeSelected={selectedTask ? canResumeTask(selectedTask.state) : canResumeAny}
      canPauseSelected={selectedTask ? canPauseTask(selectedTask.state) : canPauseAny}
      hasSelected={selectedIds.length > 0 || !!selectedId}
      on:add={() => {
        droppedUrl = "";
        addDialogOpen = true;
      }}
      on:openFolder={() => invoke("open_download_folder")}
      on:exit={() => invoke("quit_app")}
      on:resume={() => selectedId && handleResumeTask(selectedId)}
      on:pause={() => selectedId && handlePauseTask(selectedId)}
      on:pauseAll={handlePauseAll}
      on:resumeAll={handleResumeAll}
      on:delete={() => handleTriggerDelete()}
      on:deleteCompleted={handleDeleteCompleted}
      on:properties={() => handleOpenProperties()}
      on:selectAll={handleSelectAll}
      on:setViewMode={(e) => handleSetViewMode(e.detail)}
      on:toggleSidebar={() => handleToggleSidebar(!settings.sidebar_collapsed)}
      on:toggleToolbar={() => (showToolbar = !showToolbar)}
      on:toggleStatusbar={() => (showStatusbar = !showStatusbar)}
      on:toggleLanguage={toggleLanguage}
      on:openScheduler={() => (schedulerOpen = true)}
      on:openSettings={() => {
        settingsInitialSection = "general";
        settingsOpen = true;
      }}
      on:startTour={handleReplayTour}
      on:openExtensionGuide={() => (extensionGuideOpen = true)}
      on:about={() => {
        settingsInitialSection = "about";
        settingsOpen = true;
      }}
    />

    <!-- Native Desktop Toolbar (Requirement 3) -->
    {#if showToolbar}
      <Toolbar
        {selectedTask}
        selectedCount={selectedIds.length}
        {canPauseAny}
        {canResumeAny}
        bind:searchQuery
        theme={settings.theme}
        {autoShutdown}
        on:add={() => {
          droppedUrl = "";
          addDialogOpen = true;
        }}
        on:resume={() => selectedId && handleResumeTask(selectedId)}
        on:pause={() => selectedId && handlePauseTask(selectedId)}
        on:pauseAll={handlePauseAll}
        on:delete={() => handleTriggerDelete()}
        on:deleteCompleted={handleDeleteCompleted}
        on:schedule={() => (schedulerOpen = true)}
        on:settings={() => {
          settingsInitialSection = "general";
          settingsOpen = true;
        }}
        on:search={(e) => (searchQuery = e.detail)}
      />
    {/if}

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
            <strong class="countdown-num"><bdi dir="ltr">{shutdownSecondsLeft}</bdi></strong>
            {$t("shutdown.secondsSuffix")}
          </span>
        </div>
        <button class="btn btn-cancel-shutdown" on:click={handleCancelShutdown}>
          {$t("shutdown.cancelShutdown")}
        </button>
      </div>
    {/if}

    <!-- Main Workspace Area: Category Tree + Tasks Grid -->
    <div class="app-workspace">
      {#if !settings.sidebar_collapsed}
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
      {/if}

      <main id="tour-task-area" class="app-main">
        <TaskList
          tasks={filteredTasks}
          totalTasksCount={tasks.length}
          {selectedId}
          {selectedIds}
          viewMode={settings.view_mode || "table"}
          on:add={() => {
            droppedUrl = "";
            addDialogOpen = true;
          }}
          on:select={(e) => handleSelectTask(e.detail)}
          on:pause={(e) => handlePauseTask(e.detail)}
          on:resume={(e) => handleResumeTask(e.detail)}
          on:retry={(e) => handleRetryTask(e.detail)}
          on:cancel={(e) => handleCancelTask(e.detail)}
          on:delete={(e) => handleTriggerDelete(e.detail)}
          on:reveal={(e) => handleRevealTask(e.detail)}
          on:openFile={(e) => handleOpenFileTask(e.detail)}
          on:properties={(e) => handleOpenProperties(e.detail)}
        />
      </main>
    </div>

    <!-- Multi-pane Status Bar (Requirement 5) -->
    {#if showStatusbar}
      <footer class="app-statusbar app-summary-bar" id="tour-status-bar" role="status">
        <!-- Panel 1: المهام النشطة: X من Y -->
        <div class="status-pane panel-active">
          <span>{$t("statusbar.activeTasks").replace("{active}", String(activeTasksCount)).replace("{total}", String(tasks.length))}</span>
        </div>

        <!-- Panel 2: السرعة الإجمالية: XX.X MB/s -->
        <div class="status-pane panel-speed">
          <span>{$t("statusbar.totalSpeed")}</span>
          <span class="speed-indicator"><bdi dir="ltr">{formatSpeed(totalSpeedBps, $currentLang)}</bdi></span>
        </div>

        <!-- Panel 3: المساحة الحرة على C: XXX GB -->
        <div class="status-pane panel-disk">
          <span>{$t("statusbar.diskSpace").replace("{space}", "")}<bdi dir="ltr">142.8 GB</bdi></span>
        </div>

        <!-- Panel 4: متصل (المضيف الأصلي نشط) -->
        <div class="status-pane panel-bridge status-pane-flex">
          <span class="bridge-indicator-dot" aria-hidden="true"></span>
          <span>{$t("statusbar.bridgeConnected")}</span>
        </div>

        {#if settings.prevent_sleep && activeTasksCount > 0}
          <div class="status-pane status-indicator" title={$t("summary.preventSleep")}>
            <span class="indicator-dot dot-sleep"></span>
            <span>{$t("summary.preventSleep")}</span>
          </div>
        {/if}

        {#if autoShutdown}
          <div class="status-pane status-indicator" class:armed={shutdownArmed}>
            <span class="indicator-dot dot-shutdown"></span>
            <span>{shutdownArmed ? $t("summary.shutdownArmed") : $t("summary.autoShutdown")}</span>
          </div>
        {/if}
      </footer>
    {/if}
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

  <!-- Delete Confirmation Dialog (IDM style) -->
  <DeleteConfirmDialog
    open={deleteDialogOpen}
    count={deleteTaskIds.length}
    filename={deleteDialogFilename}
    on:confirm={handleConfirmDelete}
    on:cancel={handleCancelDeleteDialog}
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

  <!-- Scheduler Dialog (Bug 2) -->
  <SchedulerDialog
    open={schedulerOpen}
    on:close={() => (schedulerOpen = false)}
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

  .app-titlebar {
    height: 30px;
    background: #f9f9fb;
    border-bottom: 1px solid #e5e5e5;
    display: flex;
    align-items: center;
    padding: 0 10px;
    user-select: none;
    -webkit-user-select: none;
    flex-shrink: 0;
  }

  :global([data-theme="dark"]) .app-titlebar {
    background: #070f2e;
    border-bottom-color: #162b6e;
  }

  .titlebar-brand {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .titlebar-logo {
    width: 20px;
    height: 20px;
    object-fit: contain;
  }

  .titlebar-title {
    font-size: 12px;
    font-weight: 600;
    color: #0b1a4e;
  }

  :global([data-theme="dark"]) .titlebar-title {
    color: #f1f5f9;
  }

  .titlebar-version {
    font-size: 11px;
    color: #6b7280;
  }

  :global([data-theme="dark"]) .titlebar-version {
    color: #94a3b8;
  }

  .app-workspace {
    flex: 1;
    display: flex;
    overflow: hidden;
    min-height: 0;
    background: #ffffff;
  }

  :global([data-theme="dark"]) .app-workspace {
    background: #070f2e;
  }

  .app-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow-y: hidden;
    overflow-x: auto;
    min-width: 0;
  }

  /* Multi-pane Sunken Status Bar (Requirement 5) */
  .app-statusbar {
    height: 24px;
    background: #f6f7f9;
    border-top: 1px solid #dcdcdc;
    display: flex;
    align-items: center;
    padding: 0 4px;
    gap: 2px;
    font-size: 11.5px;
    color: #4b5563;
    flex-shrink: 0;
    user-select: none;
    -webkit-user-select: none;
  }

  :global([data-theme="dark"]) .app-statusbar {
    background: #09143c;
    border-top-color: #162b6e;
    color: #cbd5e1;
  }

  .status-pane {
    display: flex;
    align-items: center;
    padding: 0 8px;
    border-inline-end: 1px solid #dcdcdc;
    height: 18px;
    gap: 6px;
    white-space: nowrap;
    font-size: 11.5px;
  }

  :global([data-theme="dark"]) .status-pane {
    border-inline-end-color: #162b6e;
  }

  .status-pane-flex {
    flex: 1;
  }

  .speed-indicator {
    color: #008f95;
    font-weight: 700;
  }

  :global([data-theme="dark"]) .speed-indicator {
    color: #00b6bd;
  }

  .bridge-indicator-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #16a34a;
    display: inline-block;
  }

  .status-indicator {
    font-weight: 600;
  }

  .dot-sleep {
    color: var(--state-running, #00b6bd);
  }

  .dot-shutdown {
    color: var(--state-paused, #d97706);
  }

  .status-indicator.armed {
    color: var(--state-failed, #dc2626);
    animation: pulse-armed 1.5s infinite;
  }

  .indicator-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background-color: currentColor;
    display: inline-block;
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
    .toast-item,
    .shutdown-countdown-banner {
      animation: none !important;
    }
    .status-indicator.armed {
      animation: none !important;
    }
  }
</style>

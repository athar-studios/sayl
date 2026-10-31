<script>
  import { t, currentLang } from "./i18n.js";
  import {
    formatBytes,
    formatSpeed,
    formatEta,
    formatPercent,
    formatDate,
    canPauseTask,
    canResumeTask,
    isTaskPending,
  } from "./format.js";
  import { friendlyError } from "./errors.js";
  import { taskPercent } from "./downloadWindowUi.js";
  import { createEventDispatcher, onMount } from "svelte";

  export let tasks = [];
  export let selectedId = null;
  export let selectedIds = [];
  export let pendingStart = null;

  const dispatch = createEventDispatcher();

  const STORAGE_WIDTHS_KEY = "sayl_col_widths_v2";

  // Default widths in px
  const DEFAULT_WIDTHS = {
    name: 260,
    size: 90,
    status: 180,
    eta: 90,
    speed: 100,
    date: 130,
    desc: 220,
  };

  let colWidths = { ...DEFAULT_WIDTHS };

  // Sorting
  let sortColumn = "date";
  let sortDirection = "desc"; // 'asc' | 'desc'

  // Context Menu state
  let contextMenuOpen = false;
  let contextMenuX = 0;
  let contextMenuY = 0;
  let contextMenuTask = null;

  // Column Resizing state
  let isResizing = false;
  let resizingCol = null;
  let startX = 0;
  let startWidth = 0;

  onMount(() => {
    try {
      const saved = localStorage.getItem(STORAGE_WIDTHS_KEY);
      if (saved) {
        const parsed = JSON.parse(saved);
        if (parsed && typeof parsed === "object") {
          colWidths = { ...DEFAULT_WIDTHS, ...parsed };
        }
      }
    } catch {}
  });

  function saveColWidths() {
    try {
      localStorage.setItem(STORAGE_WIDTHS_KEY, JSON.stringify(colWidths));
    } catch {}
  }

  function startResize(e, colKey) {
    e.preventDefault();
    e.stopPropagation();
    isResizing = true;
    resizingCol = colKey;
    startX = e.clientX;
    startWidth = colWidths[colKey] || DEFAULT_WIDTHS[colKey];

    window.addEventListener("mousemove", handleResizeMove);
    window.addEventListener("mouseup", handleResizeEnd);
  }

  function handleResizeMove(e) {
    if (!isResizing || !resizingCol) return;
    const isRtl = $currentLang === "ar";
    const delta = isRtl ? startX - e.clientX : e.clientX - startX;
    const newWidth = Math.max(50, startWidth + delta);
    colWidths[resizingCol] = newWidth;
  }

  function handleResizeEnd() {
    if (isResizing) {
      isResizing = false;
      resizingCol = null;
      window.removeEventListener("mousemove", handleResizeMove);
      window.removeEventListener("mouseup", handleResizeEnd);
      saveColWidths();
    }
  }

  function handleSort(colKey) {
    if (sortColumn === colKey) {
      sortDirection = sortDirection === "asc" ? "desc" : "asc";
    } else {
      sortColumn = colKey;
      sortDirection = colKey === "date" || colKey === "size" ? "desc" : "asc";
    }
  }

  $: isRowSelected = (id) => (selectedIds.length > 0 ? selectedIds.includes(id) : selectedId === id);

  function getPercent(task) {
    // Never show a percentage while the task is still probing the link
    // («فحص الرابط...»): the size is unknown then, so "100%" would be a lie.
    return taskPercent(task);
  }

  // Sorted tasks
  $: sortedTasks = [...tasks].sort((a, b) => {
    let cmp = 0;
    if (sortColumn === "name") {
      cmp = (a.filename || "").localeCompare(b.filename || "");
    } else if (sortColumn === "size") {
      cmp = (a.total_size || 0) - (b.total_size || 0);
    } else if (sortColumn === "status") {
      const pctA = getPercent(a) ?? -1;
      const pctB = getPercent(b) ?? -1;
      cmp = pctA !== pctB ? pctA - pctB : (a.state || "").localeCompare(b.state || "");
    } else if (sortColumn === "eta") {
      cmp = (a.eta_secs || 0) - (b.eta_secs || 0);
    } else if (sortColumn === "speed") {
      cmp = (a.speed_bps || 0) - (b.speed_bps || 0);
    } else if (sortColumn === "date") {
      const dateA = new Date(a.completed_at || a.created_at || 0).getTime();
      const dateB = new Date(b.completed_at || b.created_at || 0).getTime();
      cmp = dateA - dateB;
    }
    return sortDirection === "asc" ? cmp : -cmp;
  });

  function handleRowClick(e, task) {
    closeContextMenu();
    dispatch("select", {
      id: task.id,
      ctrlKey: e.ctrlKey || e.metaKey,
      shiftKey: e.shiftKey,
    });
  }

  function handleRowDblClick(e, task) {
    if (task.state === "Completed") {
      dispatch("openFile", task.id);
    } else {
      dispatch("properties", task.id);
    }
  }

  const MENU_WIDTH = 220;
  const MENU_HEIGHT = 280;
  let emptyMenuOpen = false;

  function calculatePosition(e) {
    const isRtl = $currentLang === "ar";
    let x;
    if (isRtl) {
      x = Math.max(8, Math.min(window.innerWidth - MENU_WIDTH - 8, e.clientX - MENU_WIDTH));
    } else {
      x = Math.max(8, Math.min(window.innerWidth - MENU_WIDTH - 8, e.clientX));
    }
    const y = Math.max(8, Math.min(window.innerHeight - MENU_HEIGHT - 8, e.clientY));
    return { x, y };
  }

  function handleContextMenu(e, task) {
    e.preventDefault();
    e.stopPropagation();

    if (!isRowSelected(task.id)) {
      dispatch("select", { id: task.id, ctrlKey: false, shiftKey: false });
    }

    closeContextMenu();
    contextMenuTask = task;
    const pos = calculatePosition(e);
    contextMenuX = pos.x;
    contextMenuY = pos.y;
    contextMenuOpen = true;
  }

  function handleEmptyContextMenu(e) {
    e.preventDefault();
    if (e.target && (e.target.closest(".grid-row") || e.target.closest(".context-menu-popup"))) {
      return;
    }
    closeContextMenu();
    const pos = calculatePosition(e);
    contextMenuX = pos.x;
    contextMenuY = pos.y;
    emptyMenuOpen = true;
  }

  function closeContextMenu() {
    contextMenuOpen = false;
    emptyMenuOpen = false;
    contextMenuTask = null;
  }

  function handleEmptyCtxAction(action) {
    closeContextMenu();
    dispatch(action);
  }

  function handleCtxAction(action, detail) {
    const task = contextMenuTask;
    closeContextMenu();
    if (!task) return;

    if (action === "copyUrl") {
      if (task.url && navigator?.clipboard) {
        navigator.clipboard.writeText(task.url);
      }
      return;
    }

    dispatch(action, detail ?? task.id);
  }

  function handleWindowKeydown(e) {
    if (e.key === "Escape" && (contextMenuOpen || emptyMenuOpen)) {
      closeContextMenu();
    }
  }
</script>

<svelte:window on:keydown={handleWindowKeydown} on:click={closeContextMenu} />

<div
  class="table-container"
  role="region"
  aria-label={$t("view.table")}
  on:contextmenu={handleEmptyContextMenu}
>
  <table class="tasks-table">
    <thead>
      <tr class="grid-header">
        <!-- 1. الاسم (Name) -->
        <th
          class="col-name"
          style="width: {colWidths.name}px;"
          scope="col"
          on:click={() => handleSort("name")}
        >
          <div class="th-content">
            <span>{$t("table.name")}</span>
            {#if sortColumn === "name"}
              <span class="sort-arrow">{sortDirection === "asc" ? "▲" : "▼"}</span>
            {/if}
          </div>
          <div
            class="col-resizer"
            on:mousedown={(e) => startResize(e, "name")}
            role="separator"
            aria-orientation="vertical"
          ></div>
        </th>

        <!-- 2. الحجم (Size) -->
        <th
          class="col-size"
          style="width: {colWidths.size}px;"
          scope="col"
          on:click={() => handleSort("size")}
        >
          <div class="th-content">
            <span>{$t("table.size")}</span>
            {#if sortColumn === "size"}
              <span class="sort-arrow">{sortDirection === "asc" ? "▲" : "▼"}</span>
            {/if}
          </div>
          <div
            class="col-resizer"
            on:mousedown={(e) => startResize(e, "size")}
            role="separator"
            aria-orientation="vertical"
          ></div>
        </th>

        <!-- 3. الحالة (Status / Progress) -->
        <th
          class="col-status"
          style="width: {colWidths.status}px;"
          scope="col"
          on:click={() => handleSort("status")}
        >
          <div class="th-content">
            <span>{$t("table.status")}</span>
            {#if sortColumn === "status"}
              <span class="sort-arrow">{sortDirection === "asc" ? "▲" : "▼"}</span>
            {/if}
          </div>
          <div
            class="col-resizer"
            on:mousedown={(e) => startResize(e, "status")}
            role="separator"
            aria-orientation="vertical"
          ></div>
        </th>

        <!-- 4. الوقت المتبقي (Time Left) -->
        <th
          class="col-eta"
          style="width: {colWidths.eta}px;"
          scope="col"
          on:click={() => handleSort("eta")}
        >
          <div class="th-content">
            <span>{$t("table.eta")}</span>
            {#if sortColumn === "eta"}
              <span class="sort-arrow">{sortDirection === "asc" ? "▲" : "▼"}</span>
            {/if}
          </div>
          <div
            class="col-resizer"
            on:mousedown={(e) => startResize(e, "eta")}
            role="separator"
            aria-orientation="vertical"
          ></div>
        </th>

        <!-- 5. سرعة النقل (Transfer Speed) -->
        <th
          class="col-speed"
          style="width: {colWidths.speed}px;"
          scope="col"
          on:click={() => handleSort("speed")}
        >
          <div class="th-content">
            <span>{$t("table.speed")}</span>
            {#if sortColumn === "speed"}
              <span class="sort-arrow">{sortDirection === "asc" ? "▲" : "▼"}</span>
            {/if}
          </div>
          <div
            class="col-resizer"
            on:mousedown={(e) => startResize(e, "speed")}
            role="separator"
            aria-orientation="vertical"
          ></div>
        </th>

        <!-- 6. آخر محاولة (Last Try / Date) -->
        <th
          class="col-date"
          style="width: {colWidths.date}px;"
          scope="col"
          on:click={() => handleSort("date")}
        >
          <div class="th-content">
            <span>{$t("table.date")}</span>
            {#if sortColumn === "date"}
              <span class="sort-arrow">{sortDirection === "asc" ? "▲" : "▼"}</span>
            {/if}
          </div>
          <div
            class="col-resizer"
            on:mousedown={(e) => startResize(e, "date")}
            role="separator"
            aria-orientation="vertical"
          ></div>
        </th>

        <!-- 7. الوصف (Description / URL) -->
        <th
          class="col-desc"
          style="width: {colWidths.desc}px;"
          scope="col"
        >
          <div class="th-content">
            <span>{$t("table.desc")}</span>
          </div>
          <div
            class="col-resizer"
            on:mousedown={(e) => startResize(e, "desc")}
            role="separator"
            aria-orientation="vertical"
          ></div>
        </th>
      </tr>
    </thead>
    <tbody>
      {#each sortedTasks as task (task.id)}
        {@const percent = getPercent(task)}
        {@const isSelected = isRowSelected(task.id)}
        {@const isPending = isTaskPending(pendingStart, task.id)}
        {@const isCompleted = task.state === "Completed"}
        {@const isFailed = task.state === "Failed"}
        {@const isDownloading = task.state === "Downloading" || task.state === "Probing" || task.state === "Verifying" || task.state === "Finalizing"}
        {@const isPaused = task.state === "Paused" || task.state === "Pausing"}
        {@const displayDate = isCompleted ? task.completed_at : task.created_at}
        {@const writtenFormatted = formatBytes(task.written || 0, $currentLang)}
        {@const totalFormatted = task.total_size ? formatBytes(task.total_size, $currentLang) : ""}

        <tr
          class="grid-row task-row"
          data-task-id={task.id}
          class:selected={isSelected}
          class:failed-row={isFailed}
          class:paused-row={isPaused}
          on:click={(e) => handleRowClick(e, task)}
          on:dblclick={(e) => handleRowDblClick(e, task)}
          on:contextmenu={(e) => handleContextMenu(e, task)}
          tabindex="0"
          on:keydown={(e) => {
            if (e.key === "Enter") handleRowDblClick(e, task);
          }}
        >
          <!-- 1. الاسم -->
          <td class="col-name cell-name" title={task.filename || ""}>
            <div class="col-filename">
              <span class="file-icon" aria-hidden="true">
                {#if isCompleted}
                  📄
                {:else if isDownloading}
                  ⏬
                {:else if isPaused}
                  ⏸
                {:else if isFailed}
                  ⚠️
                {:else}
                  📦
                {/if}
              </span>
              <span class="file-name-text"><bdi>{task.filename || "—"}</bdi></span>
            </div>
            {#if isFailed}
              {@const err = friendlyError(task, $currentLang)}
              <div class="inline-error" title={err.hint || err.title}>
                <span class="err-hint-text">{err.hint || err.title}</span>
              </div>
            {/if}
          </td>

          <!-- 2. الحجم -->
          <td class="col-size cell-size">
            <span class="size-text">
              {#if task.total_size}
                <bdi dir="ltr">{formatBytes(task.total_size, $currentLang)}</bdi>
              {:else if task.written}
                <bdi dir="ltr">{writtenFormatted}</bdi>
              {:else}
                <bdi>{$t("task.unknownSize")}</bdi>
              {/if}
            </span>
          </td>

          <!-- 3. الحالة / التقدم -->
          <td class="col-status cell-status">
            {#if isPending}
              <div class="cell-progress-container">
                <div class="cell-progress-bar" role="progressbar" aria-valuenow={percent ?? 0} aria-valuemin="0" aria-valuemax="100">
                  <div
                    class="cell-progress-fill"
                    style="width: {percent !== null ? percent + '%' : '100%'};"
                  ></div>
                </div>
                <div class="cell-progress-text">
                  <span class="cell-state-label">{$t("task.connecting")}</span>
                  {#if percent !== null}
                    <span class="cell-percent-label"><bdi dir="ltr">{percent}%</bdi></span>
                  {/if}
                </div>
              </div>
            {:else if isDownloading}
              <div class="cell-progress-container">
                <div class="cell-progress-bar" role="progressbar" aria-valuenow={percent ?? 0} aria-valuemin="0" aria-valuemax="100">
                  <div
                    class="cell-progress-fill"
                    style="width: {percent !== null ? percent + '%' : '30%'};"
                  ></div>
                </div>
                <div class="cell-progress-text">
                  <span class="cell-state-label">{$t("state." + task.state)}</span>
                  {#if percent !== null}
                    <span class="cell-percent-label"><bdi dir="ltr">{percent}%</bdi></span>
                  {/if}
                </div>
              </div>
            {:else if isPaused}
              {#if percent !== null}
                <div class="cell-progress-container">
                  <div class="cell-progress-bar paused-bar" role="progressbar" aria-valuenow={percent} aria-valuemin="0" aria-valuemax="100">
                    <div class="cell-progress-fill paused-fill" style="width: {percent}%;"></div>
                  </div>
                  <div class="cell-progress-text">
                    <span class="cell-state-label">{$t("state.Paused")}</span>
                    <span class="cell-percent-label"><bdi dir="ltr">{percent}%</bdi></span>
                  </div>
                </div>
              {:else}
                <span class="state-badge state-paused">
                  <span class="state-dot"></span>
                  {$t("state.Paused")}
                </span>
              {/if}
            {:else if isCompleted}
              <span class="state-badge state-completed">
                <span class="state-dot"></span>
                {$t("state.Completed")}
              </span>
            {:else if isFailed}
              {#if percent !== null && percent > 0}
                <div class="cell-progress-container">
                  <div class="cell-progress-bar failed-bar" role="progressbar" aria-valuenow={percent} aria-valuemin="0" aria-valuemax="100">
                    <div class="cell-progress-fill failed-fill" style="width: {percent}%;"></div>
                  </div>
                  <div class="cell-progress-text">
                    <span class="cell-state-label">{$t("state.Failed")}</span>
                    <span class="cell-percent-label"><bdi dir="ltr">{percent}%</bdi></span>
                  </div>
                </div>
              {:else}
                <span class="state-badge state-failed">
                  <span class="state-dot"></span>
                  {$t("state.Failed")}
                </span>
              {/if}
            {:else if task.state === "Queued"}
              <span class="state-badge state-queued">
                <span class="state-dot"></span>
                {$t("state.Queued")}
              </span>
            {:else}
              <span class="state-badge">
                <span class="state-dot"></span>
                {$t("state." + task.state)}
              </span>
            {/if}
          </td>

          <!-- 4. الوقت المتبقي -->
          <td class="col-eta cell-eta">
            <span class="eta-text">
              {#if isDownloading && task.eta_secs}
                <bdi dir="ltr">{formatEta(task.eta_secs, $currentLang)}</bdi>
              {:else}
                —
              {/if}
            </span>
          </td>

          <!-- 5. سرعة النقل -->
          <td class="col-speed cell-speed">
            <span class="speed-text">
              {#if isDownloading && task.speed_bps}
                <bdi dir="ltr">{formatSpeed(task.speed_bps, $currentLang)}</bdi>
              {:else}
                —
              {/if}
            </span>
          </td>

          <!-- 6. آخر محاولة -->
          <td class="col-date cell-date">
            <span class="date-text">
              <bdi>{formatDate(displayDate, $currentLang)}</bdi>
            </span>
          </td>

          <!-- 7. الوصف / الرابط -->
          <td class="col-desc cell-desc" title={task.url || ""}>
            <span class="desc-text"><bdi>{task.url || "—"}</bdi></span>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<!-- Desktop Right-Click Context Menu -->
{#if contextMenuOpen && contextMenuTask}
  {@const allowResume = canResumeTask(contextMenuTask.state)}
  {@const allowPause = canPauseTask(contextMenuTask.state)}
  {@const isCompleted = contextMenuTask.state === "Completed"}
  {@const isFailed = contextMenuTask.state === "Failed"}
  {@const isPendingCtx = isTaskPending(pendingStart, contextMenuTask.id)}

  <div
    class="context-menu-popup"
    style="top: {contextMenuY}px; left: {contextMenuX}px;"
    role="menu"
    tabindex="-1"
    on:click|stopPropagation
    on:keydown|stopPropagation
  >
    <!-- Open File -->
    <button
      class="ctx-item"
      role="menuitem"
      disabled={!isCompleted}
      on:click={() => handleCtxAction("openFile")}
    >
      <span class="ctx-label">{$t("contextMenu.openFile")}</span>
      <kbd class="ctx-shortcut">Enter</kbd>
    </button>

    <!-- Open Folder -->
    <button
      class="ctx-item"
      role="menuitem"
      on:click={() => handleCtxAction("reveal")}
    >
      <span class="ctx-label">{$t("contextMenu.openFolder")}</span>
      <kbd class="ctx-shortcut">Ctrl+O</kbd>
    </button>

    <div class="ctx-sep" role="separator"></div>

    <!-- Resume -->
    <button
      class="ctx-item"
      role="menuitem"
      disabled={!allowResume || isPendingCtx}
      on:click={() => handleCtxAction("resume")}
    >
      <span class="ctx-label">{isPendingCtx ? $t("task.connecting") : $t("contextMenu.resume")}</span>
    </button>

    <!-- Pause -->
    <button
      class="ctx-item"
      role="menuitem"
      disabled={!allowPause}
      on:click={() => handleCtxAction("pause")}
    >
      <span class="ctx-label">{$t("contextMenu.pause")}</span>
    </button>

    <!-- Retry -->
    {#if isFailed}
      <button
        class="ctx-item"
        role="menuitem"
        on:click={() => handleCtxAction("retry")}
      >
        <span class="ctx-label">{$t("contextMenu.retry")}</span>
      </button>
    {/if}

    <div class="ctx-sep" role="separator"></div>

    <!-- Delete... -->
    <button
      class="ctx-item"
      role="menuitem"
      on:click={() => handleCtxAction("delete", { id: contextMenuTask.id, deleteFiles: false })}
    >
      <span class="ctx-label">{$t("contextMenu.delete")}</span>
      <kbd class="ctx-shortcut">Del</kbd>
    </button>

    <!-- Delete with file... -->
    <button
      class="ctx-item"
      role="menuitem"
      on:click={() => handleCtxAction("delete", { id: contextMenuTask.id, deleteFiles: true })}
    >
      <span class="ctx-label">{$t("contextMenu.deleteWithFile")}</span>
      <kbd class="ctx-shortcut">Shift+Del</kbd>
    </button>

    <div class="ctx-sep" role="separator"></div>

    <!-- Copy URL -->
    <button
      class="ctx-item"
      role="menuitem"
      on:click={() => handleCtxAction("copyUrl")}
    >
      <span class="ctx-label">{$t("contextMenu.copyUrl")}</span>
      <kbd class="ctx-shortcut">Ctrl+C</kbd>
    </button>

    <!-- Properties -->
    <button
      class="ctx-item"
      role="menuitem"
      on:click={() => handleCtxAction("properties")}
    >
      <span class="ctx-label">{$t("contextMenu.properties")}</span>
      <kbd class="ctx-shortcut">Alt+Enter</kbd>
    </button>
  </div>
{/if}

<!-- Empty-Space Context Menu -->
{#if emptyMenuOpen}
  <div
    class="context-menu-popup"
    style="top: {contextMenuY}px; left: {contextMenuX}px;"
    role="menu"
    tabindex="-1"
    on:click|stopPropagation
    on:keydown|stopPropagation
  >
    <!-- Add Download -->
    <button
      class="ctx-item"
      role="menuitem"
      on:click={() => handleEmptyCtxAction("add")}
    >
      <span class="ctx-label">{$t("contextMenu.addDownload")}</span>
      <kbd class="ctx-shortcut">Ctrl+N</kbd>
    </button>

    <div class="ctx-sep" role="separator"></div>

    <!-- Resume All -->
    <button
      class="ctx-item"
      role="menuitem"
      on:click={() => handleEmptyCtxAction("resumeAll")}
    >
      <span class="ctx-label">{$t("contextMenu.resumeAll")}</span>
    </button>

    <!-- Pause All -->
    <button
      class="ctx-item"
      role="menuitem"
      on:click={() => handleEmptyCtxAction("pauseAll")}
    >
      <span class="ctx-label">{$t("contextMenu.pauseAll")}</span>
    </button>

    <div class="ctx-sep" role="separator"></div>

    <!-- Select All -->
    <button
      class="ctx-item"
      role="menuitem"
      on:click={() => handleEmptyCtxAction("selectAll")}
    >
      <span class="ctx-label">{$t("contextMenu.selectAll")}</span>
      <kbd class="ctx-shortcut">Ctrl+A</kbd>
    </button>
  </div>
{/if}

<style>
  .table-container {
    width: 100%;
    height: 100%;
    overflow-x: auto;
    overflow-y: auto;
    background: var(--color-bg-card);
    color: var(--text);
    border: none;
    user-select: none;
    -webkit-user-select: none;
  }

  .tasks-table {
    width: 100%;
    min-width: 900px;
    border-collapse: collapse;
    font-size: 13px;
    font-family: var(--font-system, "Segoe UI Variable Text", "Segoe UI", Tahoma, Arial, sans-serif);
    table-layout: fixed;
    text-align: start;
  }

  .grid-header th {
    background: var(--color-bg-subtle);
    border-bottom: 1px solid var(--color-border);
    border-inline-end: 1px solid var(--color-border-subtle);
    padding: 3px 6px;
    font-weight: 600;
    color: var(--text-muted);
    height: 28px;
    position: sticky;
    top: 0;
    z-index: 10;
    white-space: nowrap;
    user-select: none;
    cursor: default;
  }

  .grid-header th:hover {
    background: var(--color-bg-card-hover);
  }

  .th-content {
    display: flex;
    align-items: center;
    gap: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sort-arrow {
    font-size: 9px;
    color: var(--accent);
  }

  .col-resizer {
    position: absolute;
    top: 0;
    inset-inline-end: 0;
    width: 5px;
    height: 100%;
    cursor: col-resize;
    z-index: 11;
  }

  .col-resizer:hover {
    background: var(--accent);
  }

  .grid-row {
    height: 26px;
    border-bottom: 1px solid var(--color-border-subtle);
    cursor: default;
  }

  .grid-row:nth-child(even) {
    background: var(--color-bg-subtle);
  }

  .grid-row:hover {
    background: var(--color-bg-card-hover);
  }

  .grid-row.selected {
    background: var(--color-accent-light) !important;
    color: var(--accent) !important;
  }

  .grid-row td {
    padding: 2px 6px;
    border-inline-end: 1px solid var(--color-border-subtle);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    vertical-align: middle;
    font-size: 12.5px;
    unicode-bidi: isolate;
  }

  .col-filename {
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
  }

  .file-icon {
    font-size: 13px;
    flex-shrink: 0;
  }

  .file-name-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 500;
  }

  .inline-error {
    font-size: 11px;
    color: var(--color-danger);
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Inline cell progress bar (Windows 11 Style) */
  .cell-progress-container {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-width: 0;
  }

  .cell-progress-bar {
    flex: 1;
    min-width: 36px;
    height: 6px;
    background: var(--color-border);
    border-radius: 3px;
    position: relative;
    overflow: hidden;
    border: none;
    box-shadow: none;
  }

  .cell-progress-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 3px;
    transition: width 0.15s ease-out;
  }

  .cell-progress-fill.paused-fill {
    background: var(--color-text-subtle);
  }

  .cell-progress-fill.failed-fill {
    background: var(--color-danger);
  }

  /* RTL: progress fill grows from right */
  :global([dir="rtl"]) .cell-progress-bar .cell-progress-fill {
    position: absolute;
    right: 0;
    top: 0;
    bottom: 0;
    left: auto;
    border-radius: 0 3px 3px 0;
  }
  :global([dir="rtl"]) .cell-progress-bar .cell-progress-fill.paused-fill {
    border-radius: 0 3px 3px 0;
  }
  :global([dir="rtl"]) .cell-progress-bar .cell-progress-fill.failed-fill {
    border-radius: 0 3px 3px 0;
  }

  .cell-progress-text {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    font-weight: 500;
    color: var(--text);
    white-space: nowrap;
    flex-shrink: 0;
  }

  .cell-state-label {
    font-weight: 600;
  }

  .cell-percent-label {
    font-variant-numeric: tabular-nums;
  }

  /* Paused row text contrast */
  .grid-row.paused-row td,
  .grid-row.paused-row .file-name-text,
  .grid-row.paused-row .cell-size,
  .grid-row.paused-row .date-text,
  .grid-row.paused-row .desc-text {
    color: var(--text-muted);
  }

  .grid-row.paused-row.selected td,
  .grid-row.paused-row.selected .file-name-text,
  .grid-row.paused-row.selected .cell-size,
  .grid-row.paused-row.selected .date-text,
  .grid-row.paused-row.selected .desc-text {
    color: inherit;
  }

  .state-badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-weight: 600;
    font-size: 11.5px;
  }

  .state-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-text-subtle);
  }

  .state-completed {
    color: var(--color-success);
  }

  .state-completed .state-dot {
    background: var(--color-success);
  }

  .state-paused {
    color: var(--text-muted);
  }

  .state-paused .state-dot {
    background: var(--color-text-subtle);
  }

  .state-failed {
    color: var(--color-danger);
  }

  .state-failed .state-dot {
    background: var(--color-danger);
  }

  .state-queued {
    color: var(--state-queued);
  }

  .state-queued .state-dot {
    background: var(--state-queued);
  }

  .desc-text {
    color: var(--text-subtle);
    font-size: 12px;
  }

  /* Desktop Context Menu */
  .context-menu-popup {
    position: fixed;
    background: var(--color-bg-card);
    border: 1px solid var(--color-border);
    color: var(--text);
    border-radius: 4px;
    box-shadow: var(--shadow-md);
    width: 230px;
    padding: 3px 0;
    font-size: 12.5px;
    z-index: 500;
    user-select: none;
  }

  .ctx-item {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 5px 12px;
    color: inherit;
    background: transparent;
    border: none;
    font-size: 12.5px;
    font-family: inherit;
    cursor: default;
    text-align: start;
  }

  .ctx-item:hover:not(:disabled) {
    background: var(--color-bg-card-hover);
    color: var(--accent);
  }

  .ctx-item:disabled {
    opacity: 0.42;
    cursor: default;
  }

  .ctx-label {
    flex: 1;
  }

  .ctx-shortcut {
    font-size: 11px;
    color: var(--text-subtle);
    background: transparent;
    border: none;
    padding: 0;
    font-family: inherit;
  }

  .ctx-sep {
    height: 1px;
    background: var(--color-border);
    margin: 3px 0;
  }
</style>

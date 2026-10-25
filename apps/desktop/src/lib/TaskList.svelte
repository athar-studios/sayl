<script>
  import { t, currentLang } from "./i18n.js";
  import TaskCard from "./TaskCard.svelte";
  import TaskTable from "./TaskTable.svelte";
  import EmptyState from "./EmptyState.svelte";
  import { createEventDispatcher } from "svelte";

  export let tasks = [];
  export let selectedId = null;
  export let selectedIds = [];
  export let totalTasksCount = 0;
  export let viewMode = "table"; // Requirement 4: default view_mode is "table"

  const dispatch = createEventDispatcher();

  let emptyMenuOpen = false;
  let contextMenuX = 0;
  let contextMenuY = 0;

  function handleContainerContextMenu(e) {
    if (tasks.length > 0 && viewMode === "table") {
      return;
    }
    e.preventDefault();
    const isRtl = $currentLang === "ar";
    const MENU_WIDTH = 220;
    const MENU_HEIGHT = 160;
    let x;
    if (isRtl) {
      x = Math.max(8, Math.min(window.innerWidth - MENU_WIDTH - 8, e.clientX - MENU_WIDTH));
    } else {
      x = Math.max(8, Math.min(window.innerWidth - MENU_WIDTH - 8, e.clientX));
    }
    const y = Math.max(8, Math.min(window.innerHeight - MENU_HEIGHT - 8, e.clientY));
    contextMenuX = x;
    contextMenuY = y;
    emptyMenuOpen = true;
  }

  function closeEmptyMenu() {
    emptyMenuOpen = false;
  }

  function handleEmptyAction(action) {
    closeEmptyMenu();
    dispatch(action);
  }

  $: isSelected = (id) => (selectedIds.length > 0 ? selectedIds.includes(id) : selectedId === id);
</script>

<svelte:window on:click={closeEmptyMenu} on:keydown={(e) => { if (e.key === "Escape") closeEmptyMenu(); }} />

<div
  class="task-list-container"
  class:cards-mode={viewMode === "cards"}
  on:contextmenu={handleContainerContextMenu}
  role="region"
>
  {#if tasks.length === 0 && totalTasksCount === 0}
    <EmptyState on:add={() => dispatch("add")} />
  {:else if tasks.length === 0 && totalTasksCount > 0}
    <div class="empty-tab-notice">
      <p>{$t("tabs.empty")}</p>
    </div>
  {:else if viewMode === "cards"}
    <div class="task-grid" role="feed" aria-busy="false">
      {#each tasks as task (task.id)}
        <TaskCard
          {task}
          selected={isSelected(task.id)}
          on:select={(e) => dispatch("select", e.detail)}
          on:pause={(e) => dispatch("pause", e.detail)}
          on:resume={(e) => dispatch("resume", e.detail)}
          on:retry={(e) => dispatch("retry", e.detail)}
          on:cancel={(e) => dispatch("cancel", e.detail)}
          on:delete={(e) => dispatch("delete", e.detail)}
          on:reveal={(e) => dispatch("reveal", e.detail)}
          on:openFile={(e) => dispatch("openFile", e.detail)}
          on:properties={(e) => dispatch("properties", e.detail)}
        />
      {/each}
    </div>
  {:else}
    <div class="table-view-wrapper">
      <TaskTable
        {tasks}
        {selectedId}
        {selectedIds}
        on:select={(e) => dispatch("select", e.detail)}
        on:pause={(e) => dispatch("pause", e.detail)}
        on:resume={(e) => dispatch("resume", e.detail)}
        on:retry={(e) => dispatch("retry", e.detail)}
        on:cancel={(e) => dispatch("cancel", e.detail)}
        on:delete={(e) => dispatch("delete", e.detail)}
        on:reveal={(e) => dispatch("reveal", e.detail)}
        on:openFile={(e) => dispatch("openFile", e.detail)}
        on:properties={(e) => dispatch("properties", e.detail)}
        on:add={() => dispatch("add")}
        on:resumeAll={() => dispatch("resumeAll")}
        on:pauseAll={() => dispatch("pauseAll")}
        on:selectAll={() => dispatch("selectAll")}
      />
    </div>
  {/if}

  {#if emptyMenuOpen}
    <div
      class="context-menu-popup"
      style="top: {contextMenuY}px; left: {contextMenuX}px;"
      role="menu"
      tabindex="-1"
      on:click|stopPropagation
      on:keydown|stopPropagation
    >
      <button class="ctx-item" role="menuitem" on:click={() => handleEmptyAction("add")}>
        <span class="ctx-label">{$t("contextMenu.addDownload")}</span>
      </button>
      <div class="ctx-sep" role="separator"></div>
      <button class="ctx-item" role="menuitem" on:click={() => handleEmptyAction("resumeAll")}>
        <span class="ctx-label">{$t("contextMenu.resumeAll")}</span>
      </button>
      <button class="ctx-item" role="menuitem" on:click={() => handleEmptyAction("pauseAll")}>
        <span class="ctx-label">{$t("contextMenu.pauseAll")}</span>
      </button>
      <div class="ctx-sep" role="separator"></div>
      <button class="ctx-item" role="menuitem" on:click={() => handleEmptyAction("selectAll")}>
        <span class="ctx-label">{$t("contextMenu.selectAll")}</span>
      </button>
    </div>
  {/if}
</div>

<style>
  .task-list-container {
    flex: 1;
    overflow-y: auto;
    overflow-x: auto;
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    min-width: 0;
  }

  .task-list-container.cards-mode {
    padding: 1.25rem 1.5rem;
  }

  .task-grid {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    max-width: 1000px;
    margin: 0 auto;
    width: 100%;
  }

  .table-view-wrapper {
    width: 100%;
    height: 100%;
    flex: 1;
    display: flex;
    min-width: 0;
    overflow-x: auto;
  }

  .empty-tab-notice {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 1;
    padding: 3rem 1rem;
    color: var(--color-text-subtle);
    font-size: 0.95rem;
  }

  .context-menu-popup {
    position: fixed;
    background: #ffffff;
    border: 1px solid #c9c9c9;
    border-radius: 4px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.18), 0 2px 6px rgba(0, 0, 0, 0.08);
    width: 220px;
    padding: 3px 0;
    font-size: 12.5px;
    z-index: 500;
    user-select: none;
  }

  :global([data-theme="dark"]) .context-menu-popup {
    background: #0b1a4e;
    border-color: #1e3a8a;
    color: #f1f5f9;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
  }

  .ctx-item {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 12px;
    color: inherit;
    background: transparent;
    border: none;
    text-align: start;
    cursor: pointer;
    font-size: 12.5px;
  }

  .ctx-item:hover:not(:disabled) {
    background: #eef2ff;
    color: #0b1a4e;
  }

  :global([data-theme="dark"]) .ctx-item:hover:not(:disabled) {
    background: #162b6e;
    color: #ffffff;
  }

  .ctx-label {
    flex: 1;
  }

  .ctx-sep {
    height: 1px;
    background: #e5e7eb;
    margin: 3px 0;
  }

  :global([data-theme="dark"]) .ctx-sep {
    background: #1e3a8a;
  }
</style>

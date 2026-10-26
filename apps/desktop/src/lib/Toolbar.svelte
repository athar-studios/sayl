<script>
  import { t, toggleLanguage } from "./i18n.js";
  import { canPauseTask, canResumeTask, isTaskPending } from "./format.js";
  import { createEventDispatcher } from "svelte";

  export let selectedTask = null;
  export let selectedCount = 0;
  export let canPauseAny = false;
  export let canResumeAny = false;
  export let searchQuery = "";
  export let theme = "system";
  export let autoShutdown = false;
  export let pendingStart = null;

  $: void theme;
  $: void autoShutdown;

  const dispatch = createEventDispatcher();

  $: isConnecting = selectedTask ? isTaskPending(pendingStart, selectedTask.id) : false;
  $: canResumeSelected = selectedTask ? (canResumeTask(selectedTask.state) && !isConnecting) : false;
  $: canPauseSelected = selectedTask ? canPauseTask(selectedTask.state) : false;
  $: canResume = selectedTask ? canResumeSelected : canResumeAny;
  $: canPause = selectedTask ? canPauseSelected : canPauseAny;
  $: hasSelected = selectedCount > 0 || !!selectedTask;

  function handleClearSearch() {
    searchQuery = "";
    dispatch("search", "");
  }
</script>

<header class="app-toolbar" id="tour-toolbar" aria-label="Desktop Toolbar">
  <div class="toolbar-buttons-row">
    <!-- 1. إضافة رابط (Add URL) -->
    <button
      class="toolbar-btn btn-add"
      id="tour-btn-add"
      title="{$t('toolbar.addUrl')} (Ctrl+N)"
      aria-label="{$t('toolbar.addUrl')} (Ctrl+N)"
      on:click={() => dispatch("add")}
    >
      <div class="toolbar-icon icon-add" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="currentColor" width="28" height="28">
          <path d="M12 4a1 1 0 011 1v6h6a1 1 0 110 2h-6v6a1 1 0 11-2 0v-6H5a1 1 0 110-2h6V5a1 1 0 011-1z" />
        </svg>
      </div>
      <span class="toolbar-label">{$t("toolbar.addUrl")}</span>
    </button>

    <div class="toolbar-sep" aria-hidden="true"></div>

    <!-- 2. استئناف (Resume) -->
    <button
      class="toolbar-btn"
      class:disabled={!canResume || isConnecting}
      disabled={!canResume || isConnecting}
      title="{isConnecting ? $t('task.connecting') : $t('toolbar.resume')} (F5)"
      aria-label="{isConnecting ? $t('task.connecting') : $t('toolbar.resume')} (F5)"
      on:click={() => dispatch("resume")}
    >
      <div class="toolbar-icon icon-resume" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="currentColor" width="28" height="28">
          <path d="M8 5.14v13.72a1 1 0 001.5.86l11-6.86a1 1 0 000-1.72l-11-6.86A1 1 0 008 5.14z" />
        </svg>
      </div>
      <span class="toolbar-label">{isConnecting ? $t("task.connecting") : $t("toolbar.resume")}</span>
    </button>

    <!-- 3. إيقاف (Stop) -->
    <button
      class="toolbar-btn"
      class:disabled={!canPause}
      disabled={!canPause}
      title={$t("toolbar.pause")}
      aria-label={$t("toolbar.pause")}
      on:click={() => dispatch("pause")}
    >
      <div class="toolbar-icon icon-pause" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="currentColor" width="28" height="28">
          <path d="M6 5a2 2 0 012-2h1a2 2 0 012 2v14a2 2 0 01-2 2H8a2 2 0 01-2-2V5zm9 0a2 2 0 012-2h1a2 2 0 012 2v14a2 2 0 01-2 2h-1a2 2 0 01-2-2V5z" />
        </svg>
      </div>
      <span class="toolbar-label">{$t("toolbar.pause")}</span>
    </button>

    <!-- 4. إيقاف الكل (Stop All) -->
    <button
      class="toolbar-btn"
      class:disabled={!canPauseAny}
      disabled={!canPauseAny}
      title={$t("toolbar.pauseAll")}
      aria-label={$t("toolbar.pauseAll")}
      on:click={() => dispatch("pauseAll")}
    >
      <div class="toolbar-icon icon-pause-all" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="currentColor" width="28" height="28">
          <path d="M5 6a2 2 0 012-2h10a2 2 0 012 2v12a2 2 0 01-2 2H7a2 2 0 01-2-2V6z" />
        </svg>
      </div>
      <span class="toolbar-label">{$t("toolbar.pauseAll")}</span>
    </button>

    <div class="toolbar-sep" aria-hidden="true"></div>

    <!-- 5. حذف (Delete) -->
    <button
      class="toolbar-btn"
      class:disabled={!hasSelected}
      disabled={!hasSelected}
      title="{$t('toolbar.delete')} (Del)"
      aria-label="{$t('toolbar.delete')} (Del)"
      on:click={() => dispatch("delete")}
    >
      <div class="toolbar-icon icon-delete" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="currentColor" width="28" height="28">
          <path fill-rule="evenodd" d="M9 3a1 1 0 00-.8.4L7.4 5H4a1 1 0 000 2h1v12a3 3 0 003 3h8a3 3 0 003-3V7h1a1 1 0 100-2h-3.4l-.8-1.6A1 1 0 0015 3H9zm-1 6a1 1 0 012 0v8a1 1 0 11-2 0V9zm5 0a1 1 0 012 0v8a1 1 0 11-2 0V9z" clip-rule="evenodd" />
        </svg>
      </div>
      <span class="toolbar-label">{$t("toolbar.delete")}</span>
    </button>

    <!-- 6. حذف المكتمل (Delete Completed) -->
    <button
      class="toolbar-btn"
      title={$t("toolbar.deleteCompleted")}
      aria-label={$t("toolbar.deleteCompleted")}
      on:click={() => dispatch("deleteCompleted")}
    >
      <div class="toolbar-icon icon-delete-completed" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="currentColor" width="28" height="28">
          <path d="M19 3H5a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2V5a2 2 0 00-2-2zm-9 14l-4-4 1.4-1.4 2.6 2.6 6.6-6.6 1.4 1.4-8 8z" />
        </svg>
      </div>
      <span class="toolbar-label">{$t("toolbar.deleteCompleted")}</span>
    </button>

    <div class="toolbar-sep" aria-hidden="true"></div>

    <!-- 7. الإعدادات (Settings) -->
    <button
      class="toolbar-btn"
      title="{$t('toolbar.settings')} (F10)"
      aria-label="{$t('toolbar.settings')} (F10)"
      on:click={() => dispatch("settings")}
    >
      <div class="toolbar-icon icon-settings" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="currentColor" width="28" height="28">
          <path fill-rule="evenodd" d="M11.49 3.17c.38-1.56 2.6-1.56 2.98 0a1.532 1.532 0 002.286.948c1.372-.836 2.942.734 2.106 2.106.54.886.061 2.042-.947 2.287 1.561.379 1.561 2.6 0 2.978a1.532 1.532 0 00.947 2.287c.836 1.372-.734 2.942-2.106 2.106a1.532 1.532 0 00-2.287.947c-.379 1.561-2.6 1.561-2.978 0a1.533 1.533 0 00-2.287-.947c-1.372.836-2.942-.734-2.106-2.106a1.533 1.533 0 00-.947-2.287c-1.561-.379-1.561-2.6 0-2.978a1.532 1.532 0 00.947-2.287c-.836-1.372.734-2.942 2.106-2.106a1.532 1.532 0 002.287-.947zM10 12a2 2 0 104 0 2 2 0 00-4 0z" clip-rule="evenodd" />
        </svg>
      </div>
      <span class="toolbar-label">{$t("toolbar.settings")}</span>
    </button>

    <!-- 8. الجدولة (Schedule) -->
    <button
      class="toolbar-btn"
      title={$t("toolbar.schedule")}
      aria-label={$t("toolbar.schedule")}
      on:click={() => dispatch("schedule")}
    >
      <div class="toolbar-icon icon-schedule" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="currentColor" width="28" height="28">
          <path fill-rule="evenodd" d="M12 2a10 10 0 100 20 10 10 0 000-20zm1 5a1 1 0 10-2 0v5a1 1 0 00.4.8l4 3a1 1 0 001.2-1.6l-3.6-2.7V7z" clip-rule="evenodd" />
        </svg>
      </div>
      <span class="toolbar-label">{$t("toolbar.schedule")}</span>
    </button>

    <!-- Search Box on the Far Side -->
    <div class="toolbar-search">
      <div class="search-input-wrap">
        <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" class="search-icon" aria-hidden="true">
          <path fill-rule="evenodd" d="M8 4a4 4 0 100 8 4 4 0 000-8zM2 8a6 6 0 1110.89 3.476l4.817 4.817a1 1 0 01-1.414 1.414l-4.816-4.816A6 6 0 012 8z" clip-rule="evenodd" />
        </svg>
        <input
          type="text"
          class="search-input"
          placeholder={$t("toolbar.searchPlaceholder")}
          aria-label={$t("toolbar.searchPlaceholder")}
          bind:value={searchQuery}
          on:input={(e) => dispatch("search", e.target.value)}
        />
        {#if searchQuery}
          <button class="search-clear-btn" on:click={handleClearSearch} aria-label={$t("filter.clearSearch")}>
            ×
          </button>
        {/if}
      </div>
    </div>
  </div>
</header>

<style>
  .app-toolbar {
    height: 68px;
    background: #f6f7f9;
    border-bottom: 1px solid #dcdcdc;
    display: flex;
    align-items: center;
    padding: 0 8px;
    user-select: none;
    -webkit-user-select: none;
    flex-shrink: 0;
  }

  :global([data-theme="dark"]) .app-toolbar {
    background: #0b1a4e;
    border-bottom-color: #162b6e;
    color: #e2e8f0;
  }

  .toolbar-buttons-row {
    display: flex;
    align-items: center;
    width: 100%;
    gap: 4px;
  }

  .toolbar-btn {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    width: 72px;
    height: 60px;
    border: 1px solid transparent;
    border-radius: 4px;
    background: transparent;
    cursor: pointer;
    color: #333333;
    gap: 3px;
    padding: 2px 4px;
    transition: background-color 0.1s, border-color 0.1s;
  }

  :global([data-theme="dark"]) .toolbar-btn {
    color: #f1f5f9;
  }

  .toolbar-btn:hover:not(:disabled) {
    background: #e9ecf1;
    border-color: #d0d7de;
  }

  :global([data-theme="dark"]) .toolbar-btn:hover:not(:disabled) {
    background: #162b6e;
    border-color: #254499;
  }

  .toolbar-btn:active:not(:disabled) {
    background: #dde2ea;
  }

  :global([data-theme="dark"]) .toolbar-btn:active:not(:disabled) {
    background: #102361;
  }

  .toolbar-btn.disabled,
  .toolbar-btn:disabled {
    opacity: 0.42;
    cursor: default;
    pointer-events: none;
  }

  .toolbar-icon {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #2b3340;
  }

  :global([data-theme="dark"]) .toolbar-icon {
    color: #e2e8f0;
  }

  .icon-add {
    color: #008f95;
  }

  :global([data-theme="dark"]) .icon-add {
    color: #00b6bd;
  }

  .icon-resume {
    color: #16a34a;
  }

  .icon-pause {
    color: #d97706;
  }

  .icon-pause-all {
    color: #dc2626;
  }

  .icon-delete {
    color: #dc2626;
  }

  .icon-delete-completed {
    color: #0284c7;
  }

  .icon-settings,
  .icon-schedule {
    color: #4b5563;
  }

  :global([data-theme="dark"]) .icon-settings,
  :global([data-theme="dark"]) .icon-schedule {
    color: #94a3b8;
  }

  .toolbar-label {
    font-size: 11px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 68px;
  }

  .toolbar-sep {
    width: 1px;
    height: 44px;
    background: #dcdcdc;
    margin: 0 4px;
  }

  :global([data-theme="dark"]) .toolbar-sep {
    background: #162b6e;
  }

  .toolbar-search {
    margin-inline-start: auto;
    display: flex;
    align-items: center;
    padding: 0 4px;
  }

  .search-input-wrap {
    position: relative;
    display: flex;
    align-items: center;
  }

  .search-icon {
    position: absolute;
    inset-inline-start: 8px;
    color: #9ca3af;
    pointer-events: none;
  }

  .search-input {
    height: 30px;
    width: 210px;
    border: 1px solid #c5cbd3;
    border-radius: 4px;
    padding: 0 28px 0 30px;
    font-size: 12px;
    font-family: inherit;
    background: #ffffff;
    color: #1f2937;
    outline: none;
    user-select: text;
    -webkit-user-select: text;
  }

  :global([data-theme="dark"]) .search-input {
    background: #070f2e;
    border-color: #1e3a8a;
    color: #f1f5f9;
  }

  .search-input:focus {
    border-color: #00b6bd;
    box-shadow: 0 0 0 1px #00b6bd;
  }

  .search-clear-btn {
    position: absolute;
    inset-inline-end: 6px;
    background: transparent;
    border: none;
    font-size: 16px;
    line-height: 1;
    color: #9ca3af;
    cursor: default;
    padding: 0 4px;
  }

  .search-clear-btn:hover {
    color: #4b5563;
  }
</style>

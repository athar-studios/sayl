<script>
  import { t, currentLang } from "./i18n.js";
  import { createEventDispatcher } from "svelte";

  export let viewMode = "table";
  export let showSidebar = true;
  export let showToolbar = true;
  export let showStatusbar = true;
  export let canResumeSelected = false;
  export let canPauseSelected = false;
  export let hasSelected = false;

  const dispatch = createEventDispatcher();

  let activeMenu = null;

  function toggleMenu(menuName) {
    activeMenu = activeMenu === menuName ? null : menuName;
  }

  function handleMenuHover(menuName) {
    if (activeMenu !== null) {
      activeMenu = menuName;
    }
  }

  function closeMenu() {
    activeMenu = null;
  }

  function handleAction(action, detail) {
    closeMenu();
    dispatch(action, detail);
  }

  function handleWindowKeydown(e) {
    if (e.key === "Escape" && activeMenu !== null) {
      closeMenu();
    }
  }

  function handleWindowClick(e) {
    if (activeMenu !== null && !e.target.closest(".menubar")) {
      closeMenu();
    }
  }
</script>

<svelte:window on:keydown={handleWindowKeydown} on:click={handleWindowClick} />

<nav class="menubar" id="tour-menu-bar" aria-label={$t("toolbar.menu")}>
  <!-- 1. ملف (File) -->
  <div class="menu-container">
    <button
      class="menu-btn"
      class:active={activeMenu === "file"}
      on:click={() => toggleMenu("file")}
      on:mouseenter={() => handleMenuHover("file")}
      aria-expanded={activeMenu === "file"}
      aria-haspopup="true"
    >
      {$t("menu.file")}
    </button>
    {#if activeMenu === "file"}
      <div class="dropdown-menu" role="menu">
        <button class="dropdown-item" role="menuitem" on:click={() => handleAction("add")}>
          <span class="item-label">{$t("menu.fileAdd")}</span>
          <kbd class="item-shortcut">Ctrl+N</kbd>
        </button>
        <button class="dropdown-item" role="menuitem" on:click={() => handleAction("addFromClipboard")}>
          <span class="item-label">{$t("menu.fileAddFromClipboard")}</span>
          <kbd class="item-shortcut">Ctrl+V</kbd>
        </button>
        <button class="dropdown-item" role="menuitem" on:click={() => handleAction("openFolder")}>
          <span class="item-label">{$t("menu.fileOpenFolder")}</span>
          <kbd class="item-shortcut">Ctrl+O</kbd>
        </button>
        <div class="dropdown-separator" role="separator"></div>
        <button class="dropdown-item" role="menuitem" on:click={() => handleAction("exit")}>
          <span class="item-label">{$t("menu.fileExit")}</span>
          <kbd class="item-shortcut">Alt+F4</kbd>
        </button>
      </div>
    {/if}
  </div>

  <!-- 2. المهام (Tasks) -->
  <div class="menu-container">
    <button
      class="menu-btn"
      class:active={activeMenu === "tasks"}
      on:click={() => toggleMenu("tasks")}
      on:mouseenter={() => handleMenuHover("tasks")}
      aria-expanded={activeMenu === "tasks"}
      aria-haspopup="true"
    >
      {$t("menu.tasks")}
    </button>
    {#if activeMenu === "tasks"}
      <div class="dropdown-menu" role="menu">
        <button
          class="dropdown-item"
          role="menuitem"
          disabled={!canResumeSelected}
          on:click={() => handleAction("resume")}
        >
          <span class="item-label">{$t("menu.tasksResume")}</span>
          <kbd class="item-shortcut">F5</kbd>
        </button>
        <button
          class="dropdown-item"
          role="menuitem"
          disabled={!canPauseSelected}
          on:click={() => handleAction("pause")}
        >
          <span class="item-label">{$t("menu.tasksPause")}</span>
        </button>
        <button class="dropdown-item" role="menuitem" on:click={() => handleAction("pauseAll")}>
          <span class="item-label">{$t("menu.tasksPauseAll")}</span>
        </button>
        <div class="dropdown-separator" role="separator"></div>
        <button
          class="dropdown-item"
          role="menuitem"
          disabled={!hasSelected}
          on:click={() => handleAction("delete")}
        >
          <span class="item-label">{$t("menu.tasksDelete")}</span>
          <kbd class="item-shortcut">Del</kbd>
        </button>
        <button class="dropdown-item" role="menuitem" on:click={() => handleAction("deleteCompleted")}>
          <span class="item-label">{$t("menu.tasksDeleteCompleted")}</span>
        </button>
        <div class="dropdown-separator" role="separator"></div>
        <button
          class="dropdown-item"
          role="menuitem"
          disabled={!hasSelected}
          on:click={() => handleAction("properties")}
        >
          <span class="item-label">{$t("menu.tasksProperties")}</span>
          <kbd class="item-shortcut">Alt+Enter</kbd>
        </button>
      </div>
    {/if}
  </div>

  <!-- 3. التنزيلات (Downloads) -->
  <div class="menu-container">
    <button
      class="menu-btn"
      class:active={activeMenu === "downloads"}
      on:click={() => toggleMenu("downloads")}
      on:mouseenter={() => handleMenuHover("downloads")}
      aria-expanded={activeMenu === "downloads"}
      aria-haspopup="true"
    >
      {$t("menu.downloads")}
    </button>
    {#if activeMenu === "downloads"}
      <div class="dropdown-menu" role="menu">
        <button class="dropdown-item" role="menuitem" on:click={() => handleAction("resumeAll")}>
          <span class="item-label">{$t("menu.downloadsResumeAll")}</span>
        </button>
        <button class="dropdown-item" role="menuitem" on:click={() => handleAction("pauseAll")}>
          <span class="item-label">{$t("menu.downloadsPauseAll")}</span>
        </button>
        <div class="dropdown-separator" role="separator"></div>
        <button class="dropdown-item" role="menuitem" on:click={() => handleAction("selectAll")}>
          <span class="item-label">{$t("menu.downloadsSelectAll")}</span>
          <kbd class="item-shortcut">Ctrl+A</kbd>
        </button>
      </div>
    {/if}
  </div>

  <!-- 4. عرض (View) -->
  <div class="menu-container">
    <button
      class="menu-btn"
      class:active={activeMenu === "view"}
      on:click={() => toggleMenu("view")}
      on:mouseenter={() => handleMenuHover("view")}
      aria-expanded={activeMenu === "view"}
      aria-haspopup="true"
    >
      {$t("menu.view")}
    </button>
    {#if activeMenu === "view"}
      <div class="dropdown-menu" role="menu">
        <button class="dropdown-item" role="menuitemradio" aria-checked={viewMode === "table"} on:click={() => handleAction("setViewMode", "table")}>
          <span class="item-check">{viewMode === "table" ? "✓" : ""}</span>
          <span class="item-label">{$t("menu.viewGrid")}</span>
        </button>
        <button class="dropdown-item" role="menuitemradio" aria-checked={viewMode === "cards"} on:click={() => handleAction("setViewMode", "cards")}>
          <span class="item-check">{viewMode === "cards" ? "✓" : ""}</span>
          <span class="item-label">{$t("menu.viewCards")}</span>
        </button>
        <div class="dropdown-separator" role="separator"></div>
        <button class="dropdown-item" role="menuitemcheckbox" aria-checked={showSidebar} on:click={() => handleAction("toggleSidebar")}>
          <span class="item-check">{showSidebar ? "✓" : ""}</span>
          <span class="item-label">{$t("menu.viewToggleTree")}</span>
        </button>
        <button class="dropdown-item" role="menuitemcheckbox" aria-checked={showToolbar} on:click={() => handleAction("toggleToolbar")}>
          <span class="item-check">{showToolbar ? "✓" : ""}</span>
          <span class="item-label">{$t("menu.viewToggleToolbar")}</span>
        </button>
        <button class="dropdown-item" role="menuitemcheckbox" aria-checked={showStatusbar} on:click={() => handleAction("toggleStatusbar")}>
          <span class="item-check">{showStatusbar ? "✓" : ""}</span>
          <span class="item-label">{$t("menu.viewToggleStatusbar")}</span>
        </button>
        <div class="dropdown-separator" role="separator"></div>
        <button class="dropdown-item" role="menuitem" on:click={() => handleAction("toggleLanguage")}>
          <span class="item-label">{$t("menu.viewLanguage")}</span>
        </button>
      </div>
    {/if}
  </div>

  <!-- 5. أدوات (Tools) -->
  <div class="menu-container">
    <button
      class="menu-btn"
      class:active={activeMenu === "tools"}
      on:click={() => toggleMenu("tools")}
      on:mouseenter={() => handleMenuHover("tools")}
      aria-expanded={activeMenu === "tools"}
      aria-haspopup="true"
    >
      {$t("menu.tools")}
    </button>
    {#if activeMenu === "tools"}
      <div class="dropdown-menu" role="menu">
        <button class="dropdown-item" role="menuitem" on:click={() => handleAction("openScheduler")}>
          <span class="item-label">{$t("menu.toolsSchedule")}</span>
        </button>
        <button class="dropdown-item" role="menuitem" on:click={() => handleAction("openSettings")}>
          <span class="item-label">{$t("menu.toolsSettings")}</span>
          <kbd class="item-shortcut">F10</kbd>
        </button>
      </div>
    {/if}
  </div>

  <!-- 6. مساعدة (Help) -->
  <div class="menu-container">
    <button
      class="menu-btn"
      class:active={activeMenu === "help"}
      on:click={() => toggleMenu("help")}
      on:mouseenter={() => handleMenuHover("help")}
      aria-expanded={activeMenu === "help"}
      aria-haspopup="true"
    >
      {$t("menu.help")}
    </button>
    {#if activeMenu === "help"}
      <div class="dropdown-menu" role="menu">
        <button class="dropdown-item" role="menuitem" on:click={() => handleAction("startTour")}>
          <span class="item-label">{$t("menu.helpTour")}</span>
        </button>
        <button class="dropdown-item" role="menuitem" on:click={() => handleAction("openExtensionGuide")}>
          <span class="item-label">{$t("tour.step6Title")}</span>
        </button>
        <div class="dropdown-separator" role="separator"></div>
        <button class="dropdown-item" role="menuitem" on:click={() => handleAction("about")}>
          <span class="item-label">{$t("menu.helpAbout")}</span>
        </button>
      </div>
    {/if}
  </div>
</nav>

<style>
  .menubar {
    height: 26px;
    background: var(--color-bg-alt);
    color: var(--text);
    border-bottom: 1px solid var(--color-border);
    display: flex;
    align-items: center;
    padding: 0 4px;
    gap: 1px;
    font-size: 12px;
    user-select: none;
    -webkit-user-select: none;
    position: relative;
    z-index: 100;
  }

  .menu-container {
    position: relative;
  }

  .menu-btn {
    padding: 3px 8px;
    height: 22px;
    border-radius: 3px;
    cursor: default;
    color: var(--text);
    font-size: 12px;
    font-family: inherit;
    display: flex;
    align-items: center;
    border: none;
    background: transparent;
  }

  .menu-btn:hover,
  .menu-btn.active {
    background: var(--color-bg-card-hover);
  }

  .dropdown-menu {
    position: absolute;
    top: 100%;
    inset-inline-start: 0;
    background: var(--color-bg-card);
    border: 1px solid var(--color-border);
    color: var(--text);
    border-radius: 4px;
    box-shadow: var(--shadow-md);
    min-width: 220px;
    padding: 3px 0;
    z-index: 200;
    font-size: 12px;
  }

  .dropdown-item {
    width: 100%;
    display: flex;
    align-items: center;
    padding: 5px 12px;
    background: transparent;
    border: none;
    color: inherit;
    font-size: 12px;
    font-family: inherit;
    cursor: default;
    text-align: start;
    gap: 8px;
  }

  .dropdown-item:hover:not(:disabled) {
    background: var(--color-bg-card-hover);
    color: var(--accent);
  }

  .dropdown-item:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .item-check {
    width: 14px;
    font-size: 11px;
    font-weight: 700;
    color: var(--accent);
    display: inline-block;
    text-align: center;
  }

  .item-label {
    flex: 1;
    white-space: nowrap;
  }

  .item-shortcut {
    font-size: 11px;
    color: var(--text-subtle);
    background: transparent;
    border: none;
    padding: 0;
    margin-inline-start: 16px;
    font-family: inherit;
  }

  .dropdown-separator {
    height: 1px;
    background: var(--color-border);
    margin: 3px 0;
  }
</style>

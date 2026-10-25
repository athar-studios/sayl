<script>
  import { t, toggleLanguage } from "./i18n.js";
  import { canPauseTask, canResumeTask } from "./format.js";
  import { createEventDispatcher } from "svelte";
  import logoFull from "../assets/logo-full.svg";
  import logoWhite from "../assets/logo-white.svg";

  export let selectedTask = null;
  export let canPauseAny = false;
  export let canResumeAny = false;
  export let theme = "system";
  export let autoShutdown = false;

  let menuOpen = false;

  const appVersion = typeof __APP_VERSION__ !== "undefined" ? __APP_VERSION__ : "0.1.0";
  const dispatch = createEventDispatcher();

  $: canResumeSelected = selectedTask ? canResumeTask(selectedTask.state) : false;
  $: canPauseSelected = selectedTask ? canPauseTask(selectedTask.state) : false;
  $: canResume = selectedTask ? canResumeSelected : canResumeAny;
  $: hasSelectedTask = !!selectedTask;

  function cycleTheme() {
    let next = "system";
    if (theme === "system") next = "dark";
    else if (theme === "dark") next = "light";
    else next = "system";
    dispatch("theme", next);
  }

  function toggleMenu() {
    menuOpen = !menuOpen;
  }

  function handleShutdownChange(e) {
    const val = e.target.checked;
    dispatch("toggleAutoShutdown", val);
  }

  function handleWindowKeydown(e) {
    if (e.key === "Escape" && menuOpen) {
      menuOpen = false;
    }
  }

  function handleWindowClick(e) {
    if (menuOpen && !e.target.closest(".menu-container")) {
      menuOpen = false;
    }
  }
</script>

<svelte:window on:keydown={handleWindowKeydown} on:click={handleWindowClick} />

<header class="app-toolbar" aria-label="Toolbar">
  <div class="toolbar-main-actions">
    <!-- 1. Add (large, SAYL gradient background, label, Ctrl+N kbd) -->
    <button
      class="toolbar-action-btn add-btn"
      title="{$t('toolbar.add')} (Ctrl+N)"
      aria-label="{$t('toolbar.add')} (Ctrl+N)"
      on:click={() => dispatch("add")}
    >
      <div class="add-icon-wrap" aria-hidden="true">
        <svg viewBox="0 0 20 20" fill="currentColor" width="20" height="20">
          <path
            fill-rule="evenodd"
            d="M10 3a1 1 0 011 1v5h5a1 1 0 110 2h-5v5a1 1 0 11-2 0v-5H4a1 1 0 110-2h5V4a1 1 0 011-1z"
            clip-rule="evenodd"
          />
        </svg>
      </div>
      <div class="add-label-row">
        <span class="action-label add-label">{$t("toolbar.add")}</span>
        <kbd class="kbd-badge">Ctrl+N</kbd>
      </div>
    </button>

    <div class="toolbar-divider" aria-hidden="true"></div>

    <!-- 2. Resume (استئناف) -->
    <button
      class="toolbar-action-btn"
      disabled={!canResume}
      title={$t("toolbar.resume")}
      aria-label={$t("toolbar.resume")}
      on:click={() => dispatch("resume")}
    >
      <div class="action-icon-wrap icon-resume" aria-hidden="true">
        <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
          <path
            fill-rule="evenodd"
            d="M10 18a8 8 0 100-16 8 8 0 000 16zM9.555 7.168A1 1 0 008 8v4a1 1 0 001.555.832l3-2a1 1 0 000-1.664l-3-2z"
            clip-rule="evenodd"
          />
        </svg>
      </div>
      <span class="action-label">{$t("toolbar.resume")}</span>
    </button>

    <!-- 3. Pause (إيقاف) -->
    <button
      class="toolbar-action-btn"
      disabled={!canPauseSelected}
      title={$t("toolbar.pause")}
      aria-label={$t("toolbar.pause")}
      on:click={() => dispatch("pause")}
    >
      <div class="action-icon-wrap icon-pause" aria-hidden="true">
        <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
          <path
            fill-rule="evenodd"
            d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zM7 8a1 1 0 012 0v4a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v4a1 1 0 102 0V8a1 1 0 00-1-1z"
            clip-rule="evenodd"
          />
        </svg>
      </div>
      <span class="action-label">{$t("toolbar.pause")}</span>
    </button>

    <!-- 4. Pause-all (إيقاف الكل) -->
    <button
      class="toolbar-action-btn"
      disabled={!canPauseAny}
      title={$t("toolbar.pauseAll")}
      aria-label={$t("toolbar.pauseAll")}
      on:click={() => dispatch("pauseAll")}
    >
      <div class="action-icon-wrap icon-pause-all" aria-hidden="true">
        <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
          <path d="M5 4a1 1 0 00-1 1v10a1 1 0 002 0V5a1 1 0 00-1-1zm5 0a1 1 0 00-1 1v10a1 1 0 002 0V5a1 1 0 00-1-1zm5 0a1 1 0 00-1 1v10a1 1 0 002 0V5a1 1 0 00-1-1z" />
        </svg>
      </div>
      <span class="action-label">{$t("toolbar.pauseAll")}</span>
    </button>

    <!-- 5. Delete (حذف) -->
    <button
      class="toolbar-action-btn"
      disabled={!hasSelectedTask}
      title={$t("toolbar.delete")}
      aria-label={$t("toolbar.delete")}
      on:click={() => dispatch("delete")}
    >
      <div class="action-icon-wrap icon-delete" aria-hidden="true">
        <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
          <path
            fill-rule="evenodd"
            d="M9 2a1 1 0 00-.894.553L7.382 4H4a1 1 0 000 2v10a2 2 0 002 2h8a2 2 0 002-2V6a1 1 0 100-2h-3.382l-.724-1.447A1 1 0 0011 2H9zM7 8a1 1 0 012 0v6a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v6a1 1 0 102 0V8a1 1 0 00-1-1z"
            clip-rule="evenodd"
          />
        </svg>
      </div>
      <span class="action-label">{$t("toolbar.delete")}</span>
    </button>

    <!-- 6. Retry (إعادة) -->
    <button
      class="toolbar-action-btn"
      disabled={!hasSelectedTask}
      title={$t("toolbar.retry")}
      aria-label={$t("toolbar.retry")}
      on:click={() => dispatch("retry")}
    >
      <div class="action-icon-wrap icon-retry" aria-hidden="true">
        <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
          <path
            fill-rule="evenodd"
            d="M4 2a1 1 0 011 1v2.101a7.002 7.002 0 0111.601 2.566 1 1 0 11-1.885.666A5.002 5.002 0 005.999 7H9a1 1 0 010 2H4a1 1 0 01-1-1V3a1 1 0 011-1zm.008 9.057a1 1 0 011.276.61A5.002 5.002 0 0014.001 13H11a1 1 0 110-2h5a1 1 0 011 1v5a1 1 0 11-2 0v-2.101a7.002 7.002 0 01-11.601-2.566 1 1 0 01.61-1.276z"
            clip-rule="evenodd"
          />
        </svg>
      </div>
      <span class="action-label">{$t("toolbar.retry")}</span>
    </button>

    <div class="toolbar-divider" aria-hidden="true"></div>

    <!-- 7. Open folder (فتح المجلد) -->
    <button
      class="toolbar-action-btn"
      title={$t("toolbar.openFolder")}
      aria-label={$t("toolbar.openFolder")}
      on:click={() => dispatch("openFolder")}
    >
      <div class="action-icon-wrap" aria-hidden="true">
        <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
          <path
            fill-rule="evenodd"
            d="M2 6a2 2 0 012-2h4l2 2h6a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z"
            clip-rule="evenodd"
          />
        </svg>
      </div>
      <span class="action-label">{$t("toolbar.openFolder")}</span>
    </button>

    <!-- 8. Settings (الإعدادات) -->
    <button
      class="toolbar-action-btn"
      title={$t("toolbar.settings")}
      aria-label={$t("toolbar.settings")}
      on:click={() => dispatch("settings")}
    >
      <div class="action-icon-wrap" aria-hidden="true">
        <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
          <path
            fill-rule="evenodd"
            d="M11.49 3.17c-.38-1.56-2.6-1.56-2.98 0a1.532 1.532 0 01-2.286.948c-1.372-.836-2.942.734-2.106 2.106.54.886.061 2.042-.947 2.287-1.561.379-1.561 2.6 0 2.978a1.532 1.532 0 01.947 2.287c-.836 1.372.734 2.942 2.106 2.106a1.532 1.532 0 012.287.947c.379 1.561 2.6 1.561 2.978 0a1.533 1.533 0 012.287-.947c1.372.836 2.942-.734 2.106-2.106a1.533 1.533 0 01.947-2.287c1.561-.379 1.561-2.6 0-2.978a1.532 1.532 0 01-.947-2.287c.836-1.372-.734-2.942-2.106-2.106a1.532 1.532 0 01-2.287-.947zM10 13a3 3 0 100-6 3 3 0 000 6z"
            clip-rule="evenodd"
          />
        </svg>
      </div>
      <span class="action-label">{$t("toolbar.settings")}</span>
    </button>
  </div>

  <!-- Far end: Language, Theme, Overflow Menu -->
  <div class="toolbar-end-controls">
    <button
      class="lang-toggle-btn"
      title="{$t('toolbar.language')}"
      aria-label="{$t('toolbar.language')}"
      on:click={toggleLanguage}
    >
      <span>{$t("toolbar.language")}</span>
    </button>

    <button
      class="icon-button"
      title="{$t('toolbar.theme')}: {$t('toolbar.theme' + theme.charAt(0).toUpperCase() + theme.slice(1))}"
      aria-label="{$t('toolbar.theme')}"
      on:click={cycleTheme}
    >
      {#if theme === 'dark'}
        <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18" aria-hidden="true">
          <path d="M17.293 13.293A8 8 0 016.707 2.707a8.001 8.001 0 1010.586 10.586z" />
        </svg>
      {:else if theme === 'light'}
        <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18" aria-hidden="true">
          <path
            fill-rule="evenodd"
            d="M10 2a1 1 0 011 1v1a1 1 0 11-2 0V3a1 1 0 011-1zm4 8a4 4 0 11-8 0 4 4 0 018 0zm-.464 4.95l.707.707a1 1 0 001.414-1.414l-.707-.707a1 1 0 00-1.414 1.414zm2.12-10.607a1 1 0 010 1.414l-.706.707a1 1 0 11-1.414-1.414l.707-.707a1 1 0 011.414 0zM17 11a1 1 0 100-2h-1a1 1 0 100 2h1zm-7 4a1 1 0 011 1v1a1 1 0 11-2 0v-1a1 1 0 011-1zM5.05 6.464A1 1 0 106.465 5.05l-.708-.707a1 1 0 00-1.414 1.414l.707.707zm1.414 8.486l-.707.707a1 1 0 01-1.414-1.414l.707-.707a1 1 0 011.414 1.414zM4 11a1 1 0 100-2H3a1 1 0 000 2h1z"
            clip-rule="evenodd"
          />
        </svg>
      {:else}
        <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18" aria-hidden="true">
          <path
            fill-rule="evenodd"
            d="M3 5a2 2 0 012-2h10a2 2 0 012 2v8a2 2 0 01-2 2h-2.22l.123.489.804.804A1 1 0 0113 18H7a1 1 0 01-.707-1.707l.804-.804L7.22 15H5a2 2 0 01-2-2V5zm2 0v8h10V5H5z"
            clip-rule="evenodd"
          />
        </svg>
      {/if}
    </button>

    <!-- Main Menu Dropdown (⋮) -->
    <div class="menu-container">
      <button
        class="icon-button menu-btn"
        class:active={menuOpen}
        title={$t("toolbar.menu")}
        aria-label={$t("toolbar.menu")}
        aria-expanded={menuOpen}
        aria-haspopup="true"
        on:click|stopPropagation={toggleMenu}
      >
        <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18" aria-hidden="true">
          <path d="M10 6a2 2 0 110-4 2 2 0 010 4zM10 12a2 2 0 110-4 2 2 0 010 4zM10 18a2 2 0 110-4 2 2 0 010 4z" />
        </svg>
      </button>

      {#if menuOpen}
        <div class="dropdown-menu" role="menu" tabindex="-1" on:click|stopPropagation on:keydown={(e) => { if (e.key === 'Escape') menuOpen = false; }}>
          <div class="menu-item-row">
            <label class="menu-toggle-label">
              <input
                type="checkbox"
                class="menu-checkbox"
                checked={autoShutdown}
                on:change={handleShutdownChange}
              />
              <span class="menu-toggle-text">{$t("shutdown.toggle")}</span>
            </label>
          </div>
        </div>
      {/if}
    </div>
  </div>
</header>

<style>
  .app-toolbar {
    min-height: 4.5rem;
    background-color: var(--color-bg-card);
    border-bottom: 1px solid var(--color-border);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.35rem 1rem;
    gap: 0.75rem;
    flex-shrink: 0;
    user-select: none;
  }

  .toolbar-main-actions {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    flex-wrap: wrap;
  }

  .toolbar-divider {
    width: 1px;
    height: 2.25rem;
    background-color: var(--color-border);
    margin: 0 0.25rem;
    flex-shrink: 0;
  }

  /* Vertical icon + label buttons */
  .toolbar-action-btn {
    display: inline-flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.25rem;
    min-width: 4.25rem;
    padding: 0.4rem 0.5rem;
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
    background: transparent;
    transition: background-color var(--transition-normal), color var(--transition-normal), opacity var(--transition-normal);
    cursor: pointer;
    font-size: 0.725rem;
    font-weight: 600;
    line-height: 1.1;
    text-align: center;
  }

  .toolbar-action-btn:hover:not(:disabled) {
    background-color: var(--color-bg-alt);
    color: var(--color-text);
  }

  .toolbar-action-btn:active:not(:disabled) {
    transform: translateY(1px);
  }

  .toolbar-action-btn:disabled {
    opacity: 0.38;
    cursor: not-allowed !important;
  }

  .action-icon-wrap {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 1.6rem;
    height: 1.6rem;
    color: inherit;
  }

  .icon-resume {
    color: var(--state-completed, #16a34a);
  }

  .toolbar-action-btn:disabled .icon-resume,
  .toolbar-action-btn:disabled .icon-pause,
  .toolbar-action-btn:disabled .icon-pause-all,
  .toolbar-action-btn:disabled .icon-delete,
  .toolbar-action-btn:disabled .icon-retry {
    color: inherit;
  }

  .icon-pause {
    color: var(--state-paused, #d97706);
  }

  .icon-pause-all {
    color: var(--color-text-muted);
  }

  .icon-delete {
    color: var(--state-failed, #dc2626);
  }

  .icon-retry {
    color: var(--color-accent);
  }

  .action-label {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 5.5rem;
  }

  /* Prominent Add Button with SAYL Gradient */
  .add-btn {
    background: var(--color-accent-gradient) !important;
    color: #ffffff !important;
    padding: 0.5rem 0.85rem;
    min-width: 6.5rem;
    border-radius: var(--radius-md);
    box-shadow: 0 2px 6px rgba(0, 182, 189, 0.3);
    flex-direction: column;
    gap: 0.2rem;
  }

  .add-btn:hover:not(:disabled) {
    filter: brightness(1.08);
    color: #ffffff !important;
  }

  .add-btn:active:not(:disabled) {
    filter: brightness(0.95);
  }

  .add-icon-wrap {
    display: flex;
    align-items: center;
    justify-content: center;
    color: #ffffff;
  }

  .add-label-row {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }

  .add-label {
    color: #ffffff;
    font-size: 0.775rem;
    font-weight: 700;
  }

  .kbd-badge {
    font-size: 0.625rem;
    font-family: inherit;
    font-weight: 700;
    background-color: rgba(255, 255, 255, 0.22);
    color: #ffffff;
    padding: 0.1rem 0.3rem;
    border-radius: 4px;
    letter-spacing: 0.02em;
  }

  /* Far-end controls (Language, Theme, Menu) */
  .toolbar-end-controls {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }

  .icon-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 2.25rem;
    height: 2.25rem;
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
  }

  .icon-button:hover {
    background-color: var(--color-bg-alt);
    color: var(--color-text);
  }

  .lang-toggle-btn {
    padding: 0.35rem 0.65rem;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: 0.775rem;
    font-weight: 600;
    color: var(--color-text);
  }

  .lang-toggle-btn:hover {
    background-color: var(--color-bg-alt);
  }

  .menu-container {
    position: relative;
    display: inline-flex;
    align-items: center;
  }

  .menu-btn.active {
    background-color: var(--color-bg-alt);
    color: var(--color-text);
  }

  .dropdown-menu {
    position: absolute;
    top: calc(100% + 0.5rem);
    inset-inline-end: 0;
    min-width: 270px;
    background-color: var(--color-bg-card);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-lg);
    padding: 0.75rem 0.95rem;
    z-index: 200;
    animation: menu-in 150ms ease;
  }

  .menu-item-row {
    display: flex;
    align-items: center;
  }

  .menu-toggle-label {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    cursor: pointer;
    user-select: none;
    font-size: 0.825rem;
    font-weight: 500;
    color: var(--color-text);
    line-height: 1.4;
  }

  .menu-checkbox {
    width: 1.1rem;
    height: 1.1rem;
    cursor: pointer;
    accent-color: var(--color-accent);
    flex-shrink: 0;
  }

  @keyframes menu-in {
    from {
      opacity: 0;
      transform: translateY(-4px) scale(0.97);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .dropdown-menu {
      animation: none !important;
    }
    .toolbar-action-btn {
      transition: none !important;
    }
  }
</style>

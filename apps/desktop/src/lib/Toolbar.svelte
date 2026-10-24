<script>
  import { t, toggleLanguage } from "./i18n.js";
  import { createEventDispatcher } from "svelte";
  import logoFull from "../assets/logo-full.svg";
  import logoWhite from "../assets/logo-white.svg";

  export let theme = "system";

  const appVersion = typeof __APP_VERSION__ !== "undefined" ? __APP_VERSION__ : "0.1.0";

  const dispatch = createEventDispatcher();

  function cycleTheme() {
    let next = "system";
    if (theme === "system") next = "dark";
    else if (theme === "dark") next = "light";
    else next = "system";
    dispatch("theme", next);
  }
</script>

<header class="app-toolbar">
  <div class="toolbar-start">
    <div class="brand">
      <div class="brand-logo" aria-hidden="true">
        <img src={logoFull} alt="SAYL" class="logo-img logo-light" />
        <img src={logoWhite} alt="SAYL" class="logo-img logo-dark" />
      </div>
      <div class="brand-text">
        <span class="brand-name">{$t("app.name")}</span>
        <span class="brand-badge">{appVersion}</span>
      </div>
    </div>

    <button class="btn btn-primary add-btn" on:click={() => dispatch("add")}>
      <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18" aria-hidden="true">
        <path fill-rule="evenodd" d="M10 3a1 1 0 011 1v5h5a1 1 0 110 2h-5v5a1 1 0 11-2 0v-5H4a1 1 0 110-2h5V4a1 1 0 011-1z" clip-rule="evenodd" />
      </svg>
      <span>{$t("toolbar.add")}</span>
      <kbd class="kbd-badge">Ctrl+N</kbd>
    </button>
  </div>

  <div class="toolbar-center">
    <div class="batch-controls">
      <button
        class="toolbar-btn"
        title={$t("toolbar.resumeAll")}
        aria-label={$t("toolbar.resumeAll")}
        on:click={() => dispatch("resumeAll")}
      >
        <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
          <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM9.555 7.168A1 1 0 008 8v4a1 1 0 001.555.832l3-2a1 1 0 000-1.664l-3-2z" clip-rule="evenodd" />
        </svg>
        <span class="btn-text">{$t("toolbar.resumeAll")}</span>
      </button>

      <button
        class="toolbar-btn"
        title={$t("toolbar.pauseAll")}
        aria-label={$t("toolbar.pauseAll")}
        on:click={() => dispatch("pauseAll")}
      >
        <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
          <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zM7 8a1 1 0 012 0v4a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v4a1 1 0 102 0V8a1 1 0 00-1-1z" clip-rule="evenodd" />
        </svg>
        <span class="btn-text">{$t("toolbar.pauseAll")}</span>
      </button>
    </div>
  </div>

  <div class="toolbar-end">
    <button
      class="icon-button"
      title={$t("toolbar.openFolder")}
      aria-label={$t("toolbar.openFolder")}
      on:click={() => dispatch("openFolder")}
    >
      <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18" aria-hidden="true">
        <path fill-rule="evenodd" d="M2 6a2 2 0 012-2h4l2 2h6a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" clip-rule="evenodd" />
      </svg>
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
          <path fill-rule="evenodd" d="M10 2a1 1 0 011 1v1a1 1 0 11-2 0V3a1 1 0 011-1zm4 8a4 4 0 11-8 0 4 4 0 018 0zm-.464 4.95l.707.707a1 1 0 001.414-1.414l-.707-.707a1 1 0 00-1.414 1.414zm2.12-10.607a1 1 0 010 1.414l-.706.707a1 1 0 11-1.414-1.414l.707-.707a1 1 0 011.414 0zM17 11a1 1 0 100-2h-1a1 1 0 100 2h1zm-7 4a1 1 0 011 1v1a1 1 0 11-2 0v-1a1 1 0 011-1zM5.05 6.464A1 1 0 106.465 5.05l-.708-.707a1 1 0 00-1.414 1.414l.707.707zm1.414 8.486l-.707.707a1 1 0 01-1.414-1.414l.707-.707a1 1 0 011.414 1.414zM4 11a1 1 0 100-2H3a1 1 0 000 2h1z" clip-rule="evenodd" />
        </svg>
      {:else}
        <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18" aria-hidden="true">
          <path fill-rule="evenodd" d="M3 5a2 2 0 012-2h10a2 2 0 012 2v8a2 2 0 01-2 2h-2.22l.123.489.804.804A1 1 0 0113 18H7a1 1 0 01-.707-1.707l.804-.804L7.22 15H5a2 2 0 01-2-2V5zm2 0v8h10V5H5z" clip-rule="evenodd" />
        </svg>
      {/if}
    </button>

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
      title={$t("toolbar.settings")}
      aria-label={$t("toolbar.settings")}
      on:click={() => dispatch("settings")}
    >
      <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18" aria-hidden="true">
        <path fill-rule="evenodd" d="M11.49 3.17c-.38-1.56-2.6-1.56-2.98 0a1.532 1.532 0 01-2.286.948c-1.372-.836-2.942.734-2.106 2.106.54.886.061 2.042-.947 2.287-1.561.379-1.561 2.6 0 2.978a1.532 1.532 0 01.947 2.287c-.836 1.372.734 2.942 2.106 2.106a1.532 1.532 0 012.287.947c.379 1.561 2.6 1.561 2.978 0a1.533 1.533 0 012.287-.947c1.372.836 2.942-.734 2.106-2.106a1.533 1.533 0 01.947-2.287c1.561-.379 1.561-2.6 0-2.978a1.532 1.532 0 01-.947-2.287c.836-1.372-.734-2.942-2.106-2.106a1.532 1.532 0 01-2.287-.947zM10 13a3 3 0 100-6 3 3 0 000 6z" clip-rule="evenodd" />
      </svg>
    </button>
  </div>
</header>

<style>
  .app-toolbar {
    height: 3.75rem;
    background-color: var(--color-bg-card);
    border-bottom: 1px solid var(--color-border);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 1.25rem;
    gap: 1rem;
    flex-shrink: 0;
  }

  .toolbar-start, .toolbar-center, .toolbar-end {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    margin-inline-end: 0.5rem;
  }

  .brand-logo {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 2rem;
    height: 2rem;
  }

  .logo-img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .logo-dark {
    display: none;
  }
  .logo-light {
    display: block;
  }

  :global([data-theme="dark"]) .logo-dark {
    display: block !important;
  }
  :global([data-theme="dark"]) .logo-light {
    display: none !important;
  }

  @media (prefers-color-scheme: dark) {
    :global(:root:not([data-theme="light"])) .logo-dark {
      display: block;
    }
    :global(:root:not([data-theme="light"])) .logo-light {
      display: none;
    }
  }

  .brand-text {
    display: flex;
    align-items: baseline;
    gap: 0.35rem;
  }

  .brand-name {
    font-size: 1.15rem;
    font-weight: 800;
    color: var(--color-text);
    letter-spacing: -0.02em;
  }

  .brand-badge {
    font-size: 0.65rem;
    font-weight: 700;
    padding: 0.1rem 0.35rem;
    background-color: var(--color-accent-light);
    color: var(--color-accent);
    border-radius: var(--radius-sm);
  }

  .btn {
    padding: 0.45rem 0.9rem;
    font-weight: 600;
    border-radius: var(--radius-sm);
  }

  .btn-primary {
    background: linear-gradient(135deg, #00b6bd, #0b1a4e) !important;
    color: #ffffff !important;
    box-shadow: 0 2px 4px rgba(0, 182, 189, 0.25);
    transition: filter var(--transition-normal), transform var(--transition-normal);
  }

  .btn-primary:hover {
    filter: brightness(1.08);
  }

  .btn-primary:active {
    filter: brightness(0.95);
    transform: scale(0.98);
  }

  .add-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
  }

  .kbd-badge {
    font-size: 0.675rem;
    font-family: inherit;
    background-color: rgba(255, 255, 255, 0.2);
    padding: 0.15rem 0.35rem;
    border-radius: 4px;
    margin-inline-start: 0.25rem;
    color: #ffffff;
  }

  .batch-controls {
    display: flex;
    align-items: center;
    background-color: var(--color-bg-alt);
    padding: 0.2rem;
    border-radius: var(--radius-sm);
    gap: 0.2rem;
  }

  .toolbar-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.35rem 0.65rem;
    color: var(--color-text-muted);
    font-size: 0.8rem;
    font-weight: 500;
    border-radius: var(--radius-sm);
  }

  .toolbar-btn:hover {
    background-color: var(--color-bg-card);
    color: var(--color-text);
  }

  @media (max-width: 720px) {
    .btn-text, .kbd-badge {
      display: none;
    }
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
</style>

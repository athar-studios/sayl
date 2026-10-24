<script>
  import { t, setLanguage } from "./i18n.js";
  import { safeInvoke } from "./tauri.js";
  import { DEFAULT_SETTINGS, normalizeSettings } from "./settings.js";
  import { createEventDispatcher } from "svelte";

  export let open = false;
  export let settings = { ...DEFAULT_SETTINGS };

  let localSettings = normalizeSettings(settings);
  let saving = false;
  let saveSuccess = false;
  let saveError = null;

  const dispatch = createEventDispatcher();

  $: if (open) {
    localSettings = normalizeSettings(settings);
    saveSuccess = false;
    saveError = null;
  }

  async function handleBrowseFolder() {
    saveError = null;
    try {
      if (typeof window !== "undefined" && (window.__TAURI_INTERNALS__ || window.__TAURI__)) {
        const { open: openDialog } = await import("@tauri-apps/plugin-dialog");
        const selected = await openDialog({
          directory: true,
          multiple: false,
          defaultPath: localSettings.download_dir || undefined,
        });
        if (selected && typeof selected === "string") {
          localSettings.download_dir = selected;
        }
      } else {
        // Fallback for mock preview
        await safeInvoke("open_download_folder");
      }
    } catch (err) {
      saveError = typeof err === "string" ? err : err?.message || String(err);
    }
  }

  async function handleSave() {
    saving = true;
    saveError = null;
    saveSuccess = false;
    try {
      const payload = normalizeSettings(localSettings);
      const updated = await safeInvoke("set_settings", { settings: payload });
      settings = { ...updated };
      if (updated.language) {
        setLanguage(updated.language);
      }
      dispatch("changeTheme", updated.theme);
      saveSuccess = true;
      setTimeout(() => {
        saveSuccess = false;
      }, 3000);
      dispatch("update", updated);
    } catch (err) {
      saveError = typeof err === "string" ? err : err?.message || String(err);
    } finally {
      saving = false;
    }
  }

  function handleClose() {
    dispatch("close");
  }

  function handleKeydown(e) {
    if (e.key === "Escape") {
      handleClose();
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if open}
  <div class="modal-backdrop" on:click={handleClose} on:keydown={handleKeydown} role="presentation">
    <div
      class="modal-card"
      on:click|stopPropagation
      on:keydown|stopPropagation
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-labelledby="settings-title"
    >
      <div class="modal-header">
        <h3 id="settings-title" class="modal-title">{$t("settings.title")}</h3>
        <button class="close-btn" on:click={handleClose} aria-label={$t("settings.close")}>
          <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18" aria-hidden="true">
            <path fill-rule="evenodd" d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z" clip-rule="evenodd" />
          </svg>
        </button>
      </div>

      <div class="modal-body">
        <div class="form-group">
          <label for="download-dir" class="form-label">{$t("settings.downloadDir")}</label>
          <div class="dir-input-row">
            <input
              id="download-dir"
              type="text"
              dir="ltr"
              class="form-control"
              class:has-error={saveError}
              bind:value={localSettings.download_dir}
            />
            <button class="btn btn-browse" type="button" on:click={handleBrowseFolder}>
              <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
                <path fill-rule="evenodd" d="M2 6a2 2 0 012-2h4l2 2h6a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" clip-rule="evenodd" />
              </svg>
              <span>{$t("settings.browse")}</span>
            </button>
          </div>
        </div>

        <div class="form-grid">
          <div class="form-group">
            <label for="max-tasks" class="form-label">{$t("settings.maxTasks")}</label>
            <input
              id="max-tasks"
              type="number"
              min="1"
              max="32"
              class="form-control"
              bind:value={localSettings.max_concurrent_tasks}
            />
          </div>

          <div class="form-group">
            <label for="max-conns" class="form-label">{$t("settings.maxConnections")}</label>
            <input
              id="max-conns"
              type="number"
              min="1"
              max="64"
              class="form-control"
              bind:value={localSettings.max_connections_per_task}
            />
          </div>
        </div>

        <div class="form-grid">
          <div class="form-group">
            <label for="theme-select" class="form-label">{$t("settings.theme")}</label>
            <select id="theme-select" class="form-control" bind:value={localSettings.theme}>
              <option value="system">{$t("toolbar.themeSystem")}</option>
              <option value="dark">{$t("toolbar.themeDark")}</option>
              <option value="light">{$t("toolbar.themeLight")}</option>
            </select>
          </div>

          <div class="form-group">
            <label for="lang-select" class="form-label">{$t("settings.language")}</label>
            <select id="lang-select" class="form-control" bind:value={localSettings.language}>
              <option value="ar">العربية (Arabic)</option>
              <option value="en">English (الإنجليزية)</option>
            </select>
          </div>
        </div>

        <div class="form-group">
          <label for="view-mode-select" class="form-label">{$t("settings.viewMode")}</label>
          <select id="view-mode-select" class="form-control" bind:value={localSettings.view_mode}>
            <option value="cards">{$t("settings.viewCards")}</option>
            <option value="table">{$t("settings.viewTable")}</option>
          </select>
        </div>

        <!-- Toggles Section (Sleep Prevention & Download Window) -->
        <div class="toggles-section">
          <label class="setting-toggle">
            <input type="checkbox" class="toggle-checkbox" bind:checked={localSettings.prevent_sleep} />
            <span class="toggle-label">{$t("settings.preventSleep")}</span>
          </label>

          <label class="setting-toggle">
            <input type="checkbox" class="toggle-checkbox" bind:checked={localSettings.show_download_window} />
            <span class="toggle-label">{$t("settings.showDownloadWindow")}</span>
          </label>

          <label class="setting-toggle">
            <input type="checkbox" class="toggle-checkbox" bind:checked={localSettings.close_download_window_on_finish} />
            <span class="toggle-label">{$t("settings.closeDownloadWindowOnFinish")}</span>
          </label>
        </div>

        <!-- Keyboard Shortcuts Section (Item 8) -->
        <div class="shortcuts-section">
          <h4 class="section-title">{$t("settings.shortcutsTitle")}</h4>
          <div class="shortcuts-grid">
            <div class="shortcut-item">
              <kbd class="kbd">Ctrl+N</kbd>
              <span class="shortcut-label">{$t("shortcuts.add")}</span>
            </div>
            <div class="shortcut-item">
              <kbd class="kbd">Space</kbd>
              <span class="shortcut-label">{$t("shortcuts.pauseResume")}</span>
            </div>
            <div class="shortcut-item">
              <kbd class="kbd">Delete</kbd>
              <span class="shortcut-label">{$t("shortcuts.cancel")}</span>
            </div>
            <div class="shortcut-item">
              <kbd class="kbd">Esc</kbd>
              <span class="shortcut-label">{$t("shortcuts.esc")}</span>
            </div>
          </div>
        </div>

        <!-- Inline Success / Error Banner (Item 7) -->
        {#if saveSuccess}
          <div class="feedback-banner success-banner" role="status" aria-live="polite">
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
              <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd" />
            </svg>
            <span>{$t("settings.savedNotice")}</span>
          </div>
        {/if}

        {#if saveError}
          <div class="feedback-banner error-banner" role="alert" aria-live="assertive">
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
              <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clip-rule="evenodd" />
            </svg>
            <span>{saveError}</span>
          </div>
        {/if}
      </div>

      <div class="modal-footer">
        <button class="btn btn-secondary" type="button" on:click={handleClose}>
          {$t("settings.close")}
        </button>
        <button class="btn btn-primary" type="button" disabled={saving} on:click={handleSave}>
          {#if saving}
            <span>...</span>
          {:else}
            <span>{$t("settings.save")}</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background-color: rgba(7, 15, 46, 0.65);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    padding: 1rem;
  }

  .modal-card {
    background-color: var(--color-bg-card);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    width: 100%;
    max-width: 580px;
    padding: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .modal-title {
    font-size: 1.15rem;
    font-weight: 700;
    color: var(--color-text);
  }

  .close-btn {
    width: 2rem;
    height: 2rem;
    color: var(--color-text-muted);
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close-btn:hover {
    background-color: var(--color-bg-alt);
    color: var(--color-text);
  }

  .modal-body {
    display: flex;
    flex-direction: column;
    gap: 1.15rem;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .form-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
  }

  .form-label {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--color-text);
  }

  .form-control {
    font-family: inherit;
    font-size: 0.875rem;
    color: var(--color-text);
    background-color: var(--color-bg-alt);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: 0.55rem 0.85rem;
    width: 100%;
    transition: border-color var(--transition-normal), box-shadow var(--transition-normal);
  }

  .form-control:focus {
    outline: none;
    border-color: var(--color-border-focus);
    box-shadow: 0 0 0 2px var(--color-accent-light);
  }

  .form-control.has-error {
    border-color: var(--color-danger);
    box-shadow: 0 0 0 2px var(--color-danger-light);
  }

  .dir-input-row {
    display: flex;
    gap: 0.5rem;
  }

  .btn-browse {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.55rem 0.9rem;
    background-color: var(--color-bg-alt);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-weight: 600;
    color: var(--color-text);
    white-space: nowrap;
    flex-shrink: 0;
  }

  .btn-browse:hover {
    border-color: var(--color-border-focus);
    color: var(--color-accent);
  }

  /* Shortcuts Section inside Settings (Item 8) */
  .shortcuts-section {
    border-top: 1px solid var(--color-border);
    padding-top: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
  }

  .section-title {
    font-size: 0.85rem;
    font-weight: 700;
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .shortcuts-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.5rem;
  }

  .shortcut-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.8rem;
    color: var(--color-text-muted);
  }

  .kbd {
    font-family: inherit;
    font-size: 0.725rem;
    font-weight: 600;
    background-color: var(--color-bg-alt);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    padding: 0.15rem 0.45rem;
    color: var(--color-text);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.08);
  }

  .shortcut-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Feedback Banners (Item 7) */
  .feedback-banner {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.65rem 0.85rem;
    border-radius: var(--radius-sm);
    font-size: 0.85rem;
    font-weight: 600;
    animation: fade-in 200ms ease;
  }

  @keyframes fade-in {
    from { opacity: 0; transform: translateY(-4px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .success-banner {
    background-color: rgba(34, 197, 94, 0.14);
    color: var(--color-success);
    border: 1px solid rgba(34, 197, 94, 0.25);
  }

  .error-banner {
    background-color: rgba(239, 68, 68, 0.14);
    color: var(--color-danger);
    border: 1px solid rgba(239, 68, 68, 0.25);
  }

  .modal-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.75rem;
    padding-top: 0.75rem;
    border-top: 1px solid var(--color-border);
  }

  .btn {
    padding: 0.5rem 1.15rem;
    font-weight: 600;
    border-radius: var(--radius-sm);
  }

  .btn-secondary {
    background-color: var(--color-bg-alt);
    color: var(--color-text-muted);
    border: 1px solid var(--color-border);
  }

  .btn-secondary:hover {
    color: var(--color-text);
  }

  .btn-primary {
    background: linear-gradient(135deg, #00b6bd, #0b1a4e) !important;
    color: #ffffff !important;
    box-shadow: 0 2px 4px rgba(0, 182, 189, 0.25);
  }

  .btn-primary:hover {
    filter: brightness(1.08);
  }

  .btn-primary:active {
    transform: scale(0.98);
  }

  .toggles-section {
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
    padding: 0.85rem 1rem;
    background-color: var(--color-bg-alt);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
  }

  .setting-toggle {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    cursor: pointer;
    user-select: none;
    font-size: 0.875rem;
    color: var(--color-text);
  }

  .setting-toggle .toggle-checkbox {
    width: 1.1rem;
    height: 1.1rem;
    cursor: pointer;
    accent-color: var(--color-accent);
  }

  .setting-toggle .toggle-label {
    font-weight: 500;
  }
</style>

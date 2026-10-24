<script>
  import { t, setLanguage } from "./i18n.js";
  import { safeInvoke, showToast } from "./tauri.js";
  import { createEventDispatcher } from "svelte";

  export let open = false;
  export let settings = {
    download_dir: "",
    max_concurrent_tasks: 3,
    max_connections_per_task: 4,
    theme: "system",
    language: "ar",
  };

  let localSettings = { ...settings };
  let saving = false;
  let savedSuccess = false;

  const dispatch = createEventDispatcher();

  $: if (open) {
    localSettings = { ...settings };
    savedSuccess = false;
  }

  async function handleSave() {
    saving = true;
    try {
      const updated = await safeInvoke("set_settings", { settings: localSettings });
      settings = { ...updated };
      if (updated.language) {
        setLanguage(updated.language);
      }
      dispatch("changeTheme", updated.theme);
      savedSuccess = true;
      setTimeout(() => {
        savedSuccess = false;
      }, 3000);
      dispatch("update", updated);
    } catch {
      // Error handled by safeInvoke toast
    } finally {
      saving = false;
    }
  }

  function handleOpenFolder() {
    safeInvoke("open_download_folder");
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
          <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
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
              bind:value={localSettings.download_dir}
            />
            <button class="btn btn-secondary" on:click={handleOpenFolder}>
              {$t("settings.browse")}
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
              bind:value={localSettings.max_connections_per_task}
            />
          </div>
        </div>

        <div class="form-grid">
          <div class="form-group">
            <label for="theme-select" class="form-label">{$t("settings.theme")}</label>
            <select id="theme-select" bind:value={localSettings.theme}>
              <option value="system">{$t("toolbar.themeSystem")}</option>
              <option value="dark">{$t("toolbar.themeDark")}</option>
              <option value="light">{$t("toolbar.themeLight")}</option>
            </select>
          </div>

          <div class="form-group">
            <label for="lang-select" class="form-label">{$t("settings.language")}</label>
            <select id="lang-select" bind:value={localSettings.language}>
              <option value="ar">العربية (Arabic)</option>
              <option value="en">English (الإنجليزية)</option>
            </select>
          </div>
        </div>

        {#if savedSuccess}
          <div class="success-banner">
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16">
              <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd" />
            </svg>
            <span>{$t("settings.savedNotice")}</span>
          </div>
        {/if}
      </div>

      <div class="modal-footer">
        <button class="btn btn-secondary" on:click={handleClose}>
          {$t("settings.close")}
        </button>
        <button class="btn btn-primary" disabled={saving} on:click={handleSave}>
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
    background-color: rgba(0, 0, 0, 0.45);
    backdrop-filter: blur(2px);
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
    max-width: 560px;
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
  }

  .close-btn:hover {
    background-color: var(--color-bg-alt);
    color: var(--color-text);
  }

  .modal-body {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
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

  .dir-input-row {
    display: flex;
    gap: 0.5rem;
  }

  .success-banner {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.65rem 0.85rem;
    background-color: rgba(22, 163, 74, 0.12);
    color: var(--color-success);
    border-radius: var(--radius-sm);
    font-size: 0.85rem;
    font-weight: 600;
  }

  .modal-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.75rem;
    padding-top: 0.5rem;
    border-top: 1px solid var(--color-border);
  }

  .btn {
    padding: 0.5rem 1rem;
    font-weight: 600;
  }

  .btn-secondary {
    background-color: var(--color-bg-alt);
    color: var(--color-text-muted);
  }

  .btn-secondary:hover {
    color: var(--color-text);
  }

  .btn-primary {
    background-color: var(--color-accent);
    color: var(--color-accent-text);
  }

  .btn-primary:hover {
    background-color: var(--color-accent-hover);
  }
</style>

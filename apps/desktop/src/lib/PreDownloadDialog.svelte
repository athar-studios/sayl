<script>
  import { createEventDispatcher } from "svelte";
  import { t, currentLang } from "./i18n.js";
  import { formatBytes } from "./format.js";
  import { CATEGORIES, classifyFile, getCategoryLabel } from "./classify.js";
  import { defaultSavePathForCapture } from "./capture.js";

  export let open = false;
  export let offer = null;
  export let defaultFolder = "";

  let filename = "";
  let folder = "";
  let selectedCategory = "other";
  let busy = false;
  let errorMsg = null;

  const dispatch = createEventDispatcher();

  $: if (open && offer) {
    filename = offer.filename || "download";
    selectedCategory = classifyFile(filename, null);
    folder =
      defaultFolder && defaultFolder.trim()
        ? defaultSavePathForCapture(defaultFolder, selectedCategory, filename)
        : "";
    errorMsg = null;
    busy = false;
  }

  $: categoryOptions = CATEGORIES.filter((c) => c.id !== "all");

  function handleCancel() {
    if (busy) return;
    dispatch("cancel");
  }

  async function handleBrowseFolder() {
    errorMsg = null;
    try {
      if (typeof window !== "undefined" && (window.__TAURI_INTERNALS__ || window.__TAURI__)) {
        const { open: openDialog } = await import("@tauri-apps/plugin-dialog");
        const selected = await openDialog({
          directory: true,
          multiple: false,
          defaultPath: folder || undefined,
        });
        if (selected && typeof selected === "string") {
          folder = selected;
        }
      }
    } catch (err) {
      errorMsg = typeof err === "string" ? err : err?.message || String(err);
    }
  }

  function handleConfirm() {
    if (busy || !offer) return;
    const trimmedName = (filename || "").trim();
    const trimmedFolder = (folder || "").trim();
    if (!trimmedName) {
      errorMsg = $t("preDownload.errorFilename");
      return;
    }
    if (!trimmedFolder) {
      errorMsg = $t("preDownload.errorFolder");
      return;
    }
    errorMsg = null;
    dispatch("confirm", {
      taskId: offer.task_id,
      filename: trimmedName,
      folder: trimmedFolder,
      category: selectedCategory,
      url: offer.url,
    });
  }

  function handleKeydown(e) {
    if (e.key === "Escape") {
      handleCancel();
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if open && offer}
  <div class="dialog-backdrop" on:click={handleCancel} on:keydown={handleKeydown} role="presentation">
    <div
      class="dialog-card"
      on:click|stopPropagation
      on:keydown|stopPropagation
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-labelledby="pre-dl-title"
    >
      <div class="dialog-header">
        <h3 id="pre-dl-title" class="dialog-title">{$t("preDownload.title")}</h3>
        <button class="close-btn" on:click={handleCancel} aria-label={$t("preDownload.cancel")}>
          <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18" aria-hidden="true">
            <path
              fill-rule="evenodd"
              d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z"
              clip-rule="evenodd"
            />
          </svg>
        </button>
      </div>

      <div class="dialog-body">
        <div class="form-group">
          <label for="pre-filename" class="form-label">{$t("preDownload.filename")}</label>
          <input
            id="pre-filename"
            class="form-control"
            bind:value={filename}
            type="text"
            dir="ltr"
            disabled={busy}
          />
        </div>

        <div class="form-group">
          <label for="pre-folder" class="form-label">{$t("preDownload.folder")}</label>
          <div class="folder-row">
            <input
              id="pre-folder"
              class="form-control folder-input"
              bind:value={folder}
              type="text"
              dir="ltr"
              disabled={busy}
            />
            <button type="button" class="btn btn-secondary btn-sm" on:click={handleBrowseFolder} disabled={busy}>
              {$t("preDownload.browse")}
            </button>
          </div>
        </div>

        <div class="form-group">
          <label for="pre-category" class="form-label">{$t("preDownload.category")}</label>
          <select id="pre-category" class="form-control" bind:value={selectedCategory} disabled={busy}>
            {#each categoryOptions as cat (cat.id)}
              <option value={cat.id}>{getCategoryLabel(cat.id, $currentLang)}</option>
            {/each}
          </select>
        </div>

        <div class="form-group">
          <span class="form-label">{$t("preDownload.size")}</span>
          <div class="size-readonly" dir="ltr">
            {#if offer.size != null && offer.size > 0}
              {formatBytes(offer.size, $currentLang)}
            {:else}
              {$t("preDownload.unknownSize")}
            {/if}
          </div>
        </div>

        {#if errorMsg}
          <p class="error-text">{errorMsg}</p>
        {/if}
      </div>

      <div class="dialog-actions">
        <button type="button" class="btn btn-secondary" on:click={handleCancel} disabled={busy}>
          {$t("preDownload.cancel")}
        </button>
        <button type="button" class="btn btn-primary" on:click={handleConfirm} disabled={busy}>
          {$t("preDownload.confirm")}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .dialog-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(7, 15, 46, 0.65);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1200;
    padding: 1rem;
    backdrop-filter: blur(4px);
  }

  .dialog-card {
    background: var(--color-bg-card);
    color: var(--text);
    border-radius: 12px;
    width: min(520px, 100%);
    box-shadow: var(--shadow-lg);
    border: 1px solid var(--color-border);
    border-top: 4px solid var(--accent);
    overflow: hidden;
  }

  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 1.25rem;
    border-bottom: 1px solid var(--color-border);
  }

  .dialog-title {
    margin: 0;
    font-size: 1.125rem;
    color: var(--text);
    font-weight: 700;
  }

  .close-btn {
    border: none;
    background: transparent;
    cursor: pointer;
    color: var(--text-muted);
    padding: 0.25rem;
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close-btn:hover {
    color: var(--text);
    background: var(--color-bg-card-hover);
  }

  .dialog-body {
    padding: 1rem 1.25rem 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .form-label {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--accent);
  }

  .form-control {
    background: var(--color-bg-input);
    color: var(--text);
    border: 1px solid var(--color-border);
    border-radius: 8px;
    padding: 0.5rem 0.65rem;
    font-size: 0.9375rem;
    font-family: inherit;
  }

  .form-control:focus {
    outline: none;
    border-color: var(--color-border-focus);
  }

  .folder-row {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }

  .folder-input {
    flex: 1;
    min-width: 0;
  }

  .size-readonly {
    padding: 0.5rem 0.65rem;
    background: var(--color-bg-subtle);
    border: 1px solid var(--color-border);
    border-radius: 8px;
    color: var(--text-muted);
    font-size: 0.9375rem;
  }

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    padding: 1rem 1.25rem 1.25rem;
    border-top: 1px solid var(--color-border-subtle);
  }

  .btn {
    border-radius: 8px;
    padding: 0.5rem 1rem;
    font-weight: 600;
    cursor: pointer;
    border: none;
    font-family: inherit;
    transition: all 120ms ease;
  }

  .btn-primary {
    background: var(--accent);
    color: #ffffff;
  }

  .btn-primary:hover:not(:disabled) {
    background: var(--color-accent-hover);
    color: #ffffff;
  }

  .btn-secondary {
    background: var(--color-bg-card);
    color: var(--text);
    border: 1px solid var(--color-border);
  }

  .btn-secondary:hover:not(:disabled) {
    background: var(--color-bg-card-hover);
  }

  .btn-sm {
    padding: 0.45rem 0.75rem;
    font-size: 0.875rem;
    white-space: nowrap;
  }

  .error-text {
    color: var(--color-danger);
    font-size: 0.875rem;
    margin: 0;
  }

  .btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>

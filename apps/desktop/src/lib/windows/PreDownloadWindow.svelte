<script>
  import { createEventDispatcher } from "svelte";
  import { t, currentLang } from "../i18n.js";
  import { formatBytes } from "../format.js";
  import { CATEGORIES, classifyFile, getCategoryLabel } from "../classify.js";
  import { joinCategoryFolder, sameFolderPath } from "../settings.js";
  import { safeInvoke } from "../tauri.js";
  import iconSvg from "../../assets/icon.svg";

  export let open = false;
  export let offer = null;
  export let defaultFolder = "";
  export let rememberPath = false;

  let filename = "";
  let folder = "";
  let selectedCategory = "other";
  let url = "";
  let fileSize = null;
  let busy = false;
  let errorMsg = null;

  /** Backend settings snapshot used for the category-folder prefill. */
  let settings = { download_dir: "", category_folders: {} };
  /** Set once the user types or browses a folder; the prefill then stays away. */
  let folderEdited = false;
  /** Guards against a slow prefill overwriting a newer category choice. */
  let prefillRequest = 0;

  const dispatch = createEventDispatcher();

  $: if (open && offer) {
    filename = offer.filename || "download";
    url = offer.url || "";
    fileSize = offer.size != null && offer.size > 0 ? offer.size : null;
    selectedCategory = classifyFile(filename, offer.mime || null);
    folderEdited = false;
    // Instant guess from the base folder; the async prefill refines it below.
    folder = defaultFolder ? joinCategoryFolder(defaultFolder, selectedCategory) : "";
    errorMsg = null;
    busy = false;
    bootstrapPrefill();
  }

  /** Loads the settings the backend uses to resolve category folders. */
  async function loadSettings() {
    try {
      const loaded = await safeInvoke("get_settings", {}, { suppressToast: true });
      if (loaded && typeof loaded === "object") {
        settings = { ...settings, ...loaded };
      }
    } catch {
      // Browser preview or a backend error: the local path is a good default.
    }
  }

  /**
   * Prefills the save folder from the task category.
   *
   * Priority: a remembered folder (the `defaultFolder` prop, which the app
   * derives from "remember this path") wins, then the backend-resolved category
   * folder (per-category override or `<download dir>\<subfolder>`). The backend
   * is asked so this window shows exactly the path the engine will use.
   */
  async function refreshPrefill() {
    const request = ++prefillRequest;
    const remembered = (defaultFolder || "").trim();
    const downloadDir = (settings.download_dir || "").trim();
    const base = remembered || downloadDir;
    if (!base) return;

    const local = joinCategoryFolder(base, selectedCategory);
    const keepsRemembered =
      Boolean(remembered) && (!downloadDir || !sameFolderPath(remembered, downloadDir));
    if (!folderEdited && request === prefillRequest) {
      folder = local;
    }
    if (keepsRemembered) return;

    try {
      const resolved = await safeInvoke(
        "resolve_category_folder",
        { category: selectedCategory, filename },
        { suppressToast: true }
      );
      const trimmed = typeof resolved === "string" ? resolved.trim() : "";
      if (!folderEdited && request === prefillRequest && trimmed) {
        folder = trimmed;
      }
    } catch {
      // Keep the locally computed category path.
    }
  }

  /** Loads settings first, then prefills, so category overrides are applied. */
  async function bootstrapPrefill() {
    await loadSettings();
    await refreshPrefill();
  }

  /** Re-prefills when the user picks another category. */
  function handleCategoryChange() {
    refreshPrefill();
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
          folderEdited = true;
        }
      }
    } catch (err) {
      errorMsg = typeof err === "string" ? err : err?.message || String(err);
    }
  }

  function handleConfirm(startNow = true) {
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
      startNow,
      rememberPath,
    });
  }

  function handleKeydown(e) {
    if (e.key === "Escape" && open && offer) {
      handleCancel();
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if open && offer}
  <div class="dialog-backdrop" on:click={handleCancel} role="presentation">
    <div
      class="dialog-card"
      on:click|stopPropagation
      on:keydown|stopPropagation
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-labelledby="pre-dl-title"
      class:rtl={$currentLang === "ar"}
      dir={$currentLang === "ar" ? "rtl" : "ltr"}
    >
      <!-- Header -->
      <div class="dialog-header">
        <div class="brand-wrap">
          <div class="logo-box" aria-hidden="true">
            <img src={iconSvg} alt="SAYL" class="logo-img" />
          </div>
          <h3 id="pre-dl-title" class="dialog-title">{$t("preDownload.title")}</h3>
        </div>
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

      <!-- Body -->
      <div class="dialog-body">
        <!-- URL Field (read-only) -->
        <div class="form-group">
          <label for="pre-url" class="form-label">{$t("downloadWindow.url")}</label>
          <input
            id="pre-url"
            class="form-control url-input"
            type="text"
            value={url}
            readonly
            dir="ltr"
            disabled={busy}
            title={url}
          />
        </div>

        <!-- File Name -->
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

        <!-- File Size (read-only) -->
        <div class="form-group">
          <span class="form-label">{$t("preDownload.size")}</span>
          <div class="size-readonly" dir="ltr">
            {#if fileSize != null && fileSize > 0}
              {formatBytes(fileSize, $currentLang)}
            {:else}
              {$t("preDownload.unknownSize")}
            {/if}
          </div>
        </div>

        <!-- Category -->
        <div class="form-group">
          <label for="pre-category" class="form-label">{$t("preDownload.category")}</label>
          <select
            id="pre-category"
            class="form-control"
            bind:value={selectedCategory}
            on:change={handleCategoryChange}
            disabled={busy}
          >
            {#each categoryOptions as cat (cat.id)}
              <option value={cat.id}>{getCategoryLabel(cat.id, $currentLang)}</option>
            {/each}
          </select>
        </div>

        <!-- Save As Path with Browse -->
        <div class="form-group">
          <label for="pre-folder" class="form-label">{$t("preDownload.folder")}</label>
          <div class="folder-row">
            <input
              id="pre-folder"
              class="form-control folder-input"
              bind:value={folder}
              type="text"
              dir="ltr"
              on:input={() => (folderEdited = true)}
              disabled={busy}
            />
            <button type="button" class="btn btn-secondary btn-sm" on:click={handleBrowseFolder} disabled={busy}>
              {$t("preDownload.browse")}
            </button>
          </div>
        </div>

        <!-- Remember Path Checkbox -->
        <div class="form-group checkbox-group">
          <label class="option-label">
            <input
              type="checkbox"
              class="option-checkbox"
              bind:checked={rememberPath}
              disabled={busy}
            />
            <span>{$t("preDownload.rememberPath")}</span>
          </label>
        </div>

        {#if errorMsg}
          <p class="error-text">{errorMsg}</p>
        {/if}
      </div>

      <!-- Footer Actions -->
      <div class="dialog-actions">
        <button type="button" class="btn btn-secondary" on:click={handleCancel} disabled={busy}>
          {$t("preDownload.cancel")}
        </button>
        <button type="button" class="btn btn-secondary" on:click={() => handleConfirm(false)} disabled={busy}>
          {$t("preDownload.downloadLater")}
        </button>
        <button type="button" class="btn btn-primary" on:click={() => handleConfirm(true)} disabled={busy}>
          {$t("preDownload.startDownload")}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .dialog-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(11, 26, 78, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1200;
    padding: 1rem;
    backdrop-filter: blur(4px);
  }

  .dialog-card {
    background: var(--color-bg-card);
    color: var(--color-text);
    border-radius: 12px;
    width: min(560px, 100%);
    box-shadow: var(--shadow-lg);
    border: 1px solid var(--color-border);
    /* Turquoise stays a 4px accent rule, never text behind or on the card. */
    border-top: 4px solid var(--color-accent);
    overflow: hidden;
  }

  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 1.25rem;
    border-bottom: 1px solid var(--color-border);
  }

  .brand-wrap {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .logo-box {
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .logo-img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .dialog-title {
    margin: 0;
    font-size: 1.125rem;
    color: var(--color-text);
    font-weight: 700;
  }

  .close-btn {
    border: none;
    background: transparent;
    cursor: pointer;
    color: var(--color-text-muted);
    padding: 0.25rem;
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close-btn:hover {
    color: var(--color-text);
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

  /* Labels are muted; field values keep the normal text colour. */
  .form-label {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--color-text-muted);
  }

  .form-control {
    background: var(--color-bg-input);
    color: var(--color-text);
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

  /* No opacity dimming: muted text on the subtle surface stays >= 6.8:1 light
     and >= 12:1 dark, so a disabled field is clearly inert but still readable. */
  .form-control:disabled,
  .form-control[readonly] {
    background: var(--color-bg-subtle);
    color: var(--color-text-muted);
    border-color: var(--color-text-subtle);
    border-style: dashed;
    opacity: 1;
    cursor: not-allowed;
  }

  .url-input {
    font-family: var(--font-mono, "Consolas", "Monospace");
    font-size: 0.8125rem;
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
    color: var(--color-text);
    font-size: 0.9375rem;
  }

  .checkbox-group {
    margin-top: 0.25rem;
  }

  .option-label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 0.8125rem;
    color: var(--color-text-muted);
    cursor: pointer;
  }

  .option-checkbox {
    cursor: pointer;
    accent-color: var(--color-accent);
    width: 16px;
    height: 16px;
  }

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    padding: 1rem 1.25rem 1.25rem;
    border-top: 1px solid var(--color-border-subtle);
  }

  .dialog-card.rtl .dialog-actions {
    justify-content: flex-start;
  }

  .btn {
    border-radius: 8px;
    padding: 0.5rem 1rem;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid transparent;
    font-family: inherit;
    transition: background-color 120ms ease, border-color 120ms ease;
  }

  /* Navy is the primary accent (white on #0b1a4e = 16.4:1); turquoise stays the
     card's top rule, the checkbox tick and the focus ring. */
  .btn-primary {
    background: #0b1a4e;
    color: #ffffff;
    border-color: #0b1a4e;
  }

  .btn-primary:hover:not(:disabled) {
    background: #16296b;
    border-color: #16296b;
    color: #ffffff;
  }

  .btn-secondary {
    background: var(--color-bg-card);
    color: var(--color-text);
    border: 1px solid var(--color-text-subtle);
  }

  .btn-secondary:hover:not(:disabled) {
    background: var(--color-bg-card-hover);
    border-color: var(--color-text-subtle);
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

  /* Disabled buttons keep a visible border and legible text instead of fading. */
  .btn:disabled,
  .btn:disabled:hover {
    background: var(--color-bg-subtle);
    color: var(--color-text-muted);
    border-color: var(--color-text-subtle);
    border-style: dashed;
    opacity: 1;
    cursor: not-allowed;
  }

  @media (prefers-reduced-motion: reduce) {
    .btn {
      transition: none;
    }
  }
</style>
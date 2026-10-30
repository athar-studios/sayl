<script>
  import { t, currentLang } from "./i18n.js";
  import { formatBytes } from "./format.js";
  import { nextNumberedFilename } from "./classify.js";
  import StatusPill from "./StatusPill.svelte";
  import { safeInvoke } from "./tauri.js";
  import { createEventDispatcher } from "svelte";

  export let open = false;
  export let existingTask = null;

  const dispatch = createEventDispatcher();

  /**
   * The free (IDM style) name "Download again" will use.
   *
   * The existing download already holds `<task.filename>` in its folder, so the
   * engine picks `file (1).ext` instead of overwriting it. Resolving the name in
   * the backend keeps this dialog honest about what will really be written, and
   * the engine re-checks it again at start time.
   */
  let suggestedFilename = "";
  let suggestionToken = 0;

  $: if (open && existingTask) {
    loadSuggestedFilename(existingTask);
  } else if (!open) {
    suggestedFilename = "";
  }

  async function loadSuggestedFilename(task) {
    const token = ++suggestionToken;
    // Optimistic guess: the existing download holds its own name in its folder,
    // so "Download again" lands on the next number. The backend answer below
    // replaces it whenever it can see the disk and the task list.
    suggestedFilename = nextNumberedFilename(task.filename);
    const folder = (task.folder || "").trim();
    if (!folder) {
      return;
    }
    try {
      const unique = await safeInvoke(
        "get_unique_filename",
        { folder, filename: task.filename, taskId: task.id },
        { suppressToast: true }
      );
      if (token === suggestionToken && typeof unique === "string") {
        suggestedFilename = unique;
      }
    } catch {
      // Browser preview or a backend error: keep the optimistic guess.
    }
  }

  function handleOpenExisting() {
    if (existingTask) {
      dispatch("openExisting", existingTask.id);
    }
  }

  function handleDownloadAgain() {
    if (existingTask) {
      dispatch("downloadAgain", existingTask.url);
    }
  }

  function handleCancel() {
    dispatch("cancel");
  }

  function handleKeydown(e) {
    if (e.key === "Escape") {
      handleCancel();
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if open && existingTask}
  <div
    class="dialog-backdrop"
    on:click={handleCancel}
    on:keydown={handleKeydown}
    role="presentation"
  >
    <div
      class="dialog-card"
      on:click|stopPropagation
      on:keydown|stopPropagation
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-labelledby="dup-title"
    >
      <div class="dialog-header">
        <div class="header-icon-wrap" aria-hidden="true">
          <svg viewBox="0 0 20 20" fill="currentColor" width="22" height="22">
            <path
              fill-rule="evenodd"
              d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z"
              clip-rule="evenodd"
            />
          </svg>
        </div>
        <h3 id="dup-title" class="dialog-title">{$t("duplicate.title")}</h3>
        <button class="close-btn" on:click={handleCancel} aria-label={$t("duplicate.cancel")}>
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
        <p class="dialog-desc">{$t("duplicate.desc")}</p>

        <div class="existing-task-card">
          <div class="task-info">
            <span class="file-name" title={existingTask.filename}><bdi>{existingTask.filename}</bdi></span>
            {#if suggestedFilename && suggestedFilename !== existingTask.filename}
              <span
                class="file-name-next"
                dir="ltr"
                title={suggestedFilename}
                aria-label={suggestedFilename}
              >
                <span class="next-arrow" aria-hidden="true">→</span>
                <bdi>{suggestedFilename}</bdi>
              </span>
            {/if}
            <span class="file-url" title={existingTask.url} dir="ltr">{existingTask.url}</span>
          </div>
          <div class="task-meta">
            <StatusPill state={existingTask.state} />
            {#if existingTask.total_size}
              <span class="task-size"><bdi dir="ltr">{formatBytes(existingTask.total_size, $currentLang)}</bdi></span>
            {/if}
          </div>
        </div>
      </div>

      <div class="dialog-actions">
        <button
          type="button"
          class="btn btn-primary action-btn"
          on:click={handleOpenExisting}
        >
          <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
            <path d="M10 12a2 2 0 100-4 2 2 0 000 4z" />
            <path fill-rule="evenodd" d="M.458 10C1.732 5.943 5.522 3 10 3s8.268 2.943 9.542 7c-1.274 4.057-5.064 7-9.542 7S1.732 14.057.458 10zM14 10a4 4 0 11-8 0 4 4 0 018 0z" clip-rule="evenodd" />
          </svg>
          <span>{$t("duplicate.openExisting")}</span>
        </button>

        <button
          type="button"
          class="btn btn-secondary action-btn"
          on:click={handleDownloadAgain}
        >
          <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
            <path fill-rule="evenodd" d="M4 2a1 1 0 011 1v2.101a7.002 7.002 0 0111.601 2.566 1 1 0 11-1.885.666A5.002 5.002 0 005.999 7H9a1 1 0 010 2H4a1 1 0 01-1-1V3a1 1 0 011-1zm.008 9.057a1 1 0 011.276.61A5.002 5.002 0 0014.001 13H11a1 1 0 110-2h5a1 1 0 011 1v5a1 1 0 11-2 0v-2.101a7.002 7.002 0 01-11.601-2.566 1 1 0 01.61-1.276z" clip-rule="evenodd" />
          </svg>
          <span>{$t("duplicate.downloadAgain")}</span>
        </button>

        <button
          type="button"
          class="btn btn-ghost action-btn"
          on:click={handleCancel}
        >
          <span>{$t("duplicate.cancel")}</span>
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .dialog-backdrop {
    position: fixed;
    inset: 0;
    background-color: rgba(7, 15, 46, 0.65);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 150;
    padding: 1rem;
  }

  .dialog-card {
    background-color: var(--color-bg-card);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    width: 100%;
    max-width: 520px;
    padding: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .dialog-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .header-icon-wrap {
    color: var(--color-warning);
    display: flex;
    align-items: center;
  }

  .dialog-title {
    font-size: 1.15rem;
    font-weight: 700;
    color: var(--color-text);
    flex: 1;
  }

  .close-btn {
    color: var(--color-text-muted);
    background: transparent;
    border-radius: var(--radius-sm);
    padding: 0.25rem;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .close-btn:hover {
    color: var(--color-text);
    background-color: var(--color-bg-alt);
  }

  .dialog-body {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }

  .dialog-desc {
    font-size: 0.9rem;
    color: var(--color-text-muted);
    line-height: 1.5;
  }

  .existing-task-card {
    background-color: var(--color-bg-alt);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: 0.85rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .task-info {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .file-name {
    font-weight: 700;
    font-size: 0.95rem;
    color: var(--color-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file-url {
    font-size: 0.75rem;
    color: var(--color-text-subtle);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* The name "Download again" will use: the existing name plus "(1)". */
  .file-name-next {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--color-accent);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .next-arrow {
    color: var(--color-text-subtle);
    font-weight: 700;
  }

  .task-meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    font-size: 0.8rem;
    color: var(--color-text-muted);
  }

  .dialog-actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.5rem;
    flex-wrap: wrap;
    padding-top: 0.5rem;
    border-top: 1px solid var(--color-border);
  }

  .action-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.5rem 0.9rem;
    font-weight: 600;
    font-size: 0.85rem;
    border-radius: var(--radius-sm);
  }

  .btn-primary {
    background: var(--color-accent-gradient);
    color: #ffffff;
  }

  .btn-primary:hover {
    filter: brightness(1.08);
  }

  .btn-secondary {
    background-color: var(--color-bg-alt);
    border: 1px solid var(--color-border);
    color: var(--color-text);
  }

  .btn-secondary:hover {
    background-color: var(--color-bg-card-hover);
  }

  .btn-ghost {
    background: transparent;
    color: var(--color-text-muted);
  }

  .btn-ghost:hover {
    background-color: var(--color-bg-alt);
    color: var(--color-text);
  }
</style>

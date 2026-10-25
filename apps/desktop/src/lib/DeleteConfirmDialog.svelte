<script>
  import { t } from "./i18n.js";
  import { createEventDispatcher } from "svelte";

  export let open = false;
  export let count = 1;
  export let filename = "";

  const dispatch = createEventDispatcher();

  let deleteFiles = false;

  $: if (open) {
    // Always default to OFF (unchecked) per Requirement 2
    deleteFiles = false;
  }

  $: isMultiple = count > 1;
  $: dialogTitle = isMultiple
    ? $t("deleteDialog.titleMultiple")
    : $t("deleteDialog.title");

  $: promptText = isMultiple
    ? $t("deleteDialog.promptMultiple").replace("{count}", String(count))
    : filename
      ? $t("deleteDialog.promptSingle").replace("{filename}", filename)
      : $t("deleteDialog.promptSingle").replace("'{filename}'", "").replace("'{filename}' ", "");

  $: primaryButtonLabel = deleteFiles
    ? $t("deleteDialog.deleteFileAndRemove")
    : $t("deleteDialog.removeOnly");

  function handleConfirm() {
    dispatch("confirm", { deleteFiles });
  }

  function handleCancel() {
    dispatch("cancel");
  }

  function handleKeydown(e) {
    if (!open) return;
    if (e.key === "Escape") {
      e.preventDefault();
      handleCancel();
    } else if (e.key === "Enter" && !e.shiftKey && !e.ctrlKey && !e.altKey) {
      // Allow enter to trigger primary action unless focusing cancel
      if (document.activeElement?.classList?.contains("btn-secondary")) return;
      e.preventDefault();
      handleConfirm();
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if open}
  <div
    class="dialog-backdrop"
    on:click={handleCancel}
    role="presentation"
  >
    <div
      class="dialog-card"
      on:click|stopPropagation
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-labelledby="delete-dialog-title"
    >
      <div class="dialog-header">
        <div class="header-icon-wrap" aria-hidden="true">
          <svg viewBox="0 0 20 20" fill="currentColor" width="22" height="22">
            <path
              fill-rule="evenodd"
              d="M9 2a1 1 0 00-.894.553L7.382 4H4a1 1 0 000 2v10a2 2 0 002 2h8a2 2 0 002-2V6a1 1 0 100-2h-3.382l-.724-1.447A1 1 0 0011 2H9zM7 8a1 1 0 012 0v6a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v6a1 1 0 102 0V8a1 1 0 00-1-1z"
              clip-rule="evenodd"
            />
          </svg>
        </div>
        <h3 id="delete-dialog-title" class="dialog-title">{dialogTitle}</h3>
        <button
          class="close-btn"
          on:click={handleCancel}
          aria-label={$t("deleteDialog.cancel")}
        >
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
        <p class="dialog-prompt" dir="auto">
          {promptText}
        </p>

        <label class="checkbox-option">
          <input
            type="checkbox"
            bind:checked={deleteFiles}
            class="styled-checkbox"
          />
          <span class="checkbox-text">{$t("deleteDialog.alsoDeleteFile")}</span>
        </label>
      </div>

      <div class="dialog-actions">
        <button
          type="button"
          class="btn btn-secondary"
          on:click={handleCancel}
        >
          {$t("deleteDialog.cancel")}
        </button>
        <button
          type="button"
          class="btn btn-danger"
          on:click={handleConfirm}
        >
          {primaryButtonLabel}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .dialog-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(15, 23, 42, 0.65);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    padding: 1.5rem;
    animation: fadeIn 0.15s ease-out;
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .dialog-card {
    background: var(--bg-surface, #ffffff);
    color: var(--text-primary, #0f172a);
    border-radius: 6px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.25);
    width: 100%;
    max-width: 480px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    animation: slideUp 0.18s cubic-bezier(0.16, 1, 0.3, 1);
    border: 1px solid #c5cbd3;
  }

  @keyframes slideUp {
    from {
      transform: translateY(12px) scale(0.98);
      opacity: 0;
    }
    to {
      transform: translateY(0) scale(1);
    }
  }

  .dialog-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 1.15rem 1.25rem 0.85rem;
    border-bottom: 1px solid var(--border-color, #f1f5f9);
  }

  .header-icon-wrap {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border-radius: 10px;
    background: rgba(239, 68, 68, 0.12);
    color: #ef4444;
    flex-shrink: 0;
  }

  .dialog-title {
    font-size: 1.05rem;
    font-weight: 700;
    margin: 0;
    flex: 1;
    color: var(--text-primary, #0f172a);
  }

  .close-btn {
    background: none;
    border: none;
    color: var(--text-secondary, #64748b);
    cursor: pointer;
    padding: 0.4rem;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.15s, color 0.15s;
  }

  .close-btn:hover {
    background: var(--bg-hover, #f1f5f9);
    color: var(--text-primary, #0f172a);
  }

  .dialog-body {
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 1.1rem;
  }

  .dialog-prompt {
    font-size: 0.93rem;
    line-height: 1.5;
    color: var(--text-secondary, #334155);
    margin: 0;
  }

  .checkbox-option {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    cursor: pointer;
    user-select: none;
    padding: 0.65rem 0.85rem;
    border-radius: 8px;
    background: var(--bg-alt, #f8fafc);
    border: 1px solid var(--border-color, #e2e8f0);
    transition: background 0.15s, border-color 0.15s;
  }

  .checkbox-option:hover {
    background: var(--bg-hover, #f1f5f9);
    border-color: #cbd5e1;
  }

  .styled-checkbox {
    width: 18px;
    height: 18px;
    cursor: pointer;
    accent-color: var(--accent, #2563eb);
    margin: 0;
  }

  .checkbox-text {
    font-size: 0.88rem;
    font-weight: 500;
    color: var(--text-primary, #0f172a);
  }

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 0.75rem;
    padding: 1rem 1.25rem;
    background: var(--bg-alt, #f8fafc);
    border-top: 1px solid var(--border-color, #f1f5f9);
  }

  .btn {
    padding: 0.55rem 1.15rem;
    border-radius: 8px;
    font-size: 0.88rem;
    font-weight: 600;
    cursor: pointer;
    border: none;
    transition: all 0.15s ease;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .btn-secondary {
    background: var(--bg-surface, #ffffff);
    color: var(--text-primary, #334155);
    border: 1px solid var(--border-color, #cbd5e1);
  }

  .btn-secondary:hover {
    background: var(--bg-hover, #f1f5f9);
    color: var(--text-primary, #0f172a);
  }

  .btn-danger {
    background: #ef4444;
    color: #ffffff;
  }

  .btn-danger:hover {
    background: #dc2626;
  }
</style>

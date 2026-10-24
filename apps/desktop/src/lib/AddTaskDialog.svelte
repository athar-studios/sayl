<script>
  import { t } from "./i18n.js";
  import { createEventDispatcher, onMount } from "svelte";

  export let open = false;

  let urlInput = "";
  let errorMsg = null;
  let inputElement;

  const dispatch = createEventDispatcher();

  $: if (open && inputElement) {
    setTimeout(() => {
      inputElement?.focus();
    }, 50);
  }

  function validateUrl(str) {
    const trimmed = str.trim();
    if (!trimmed) {
      return $t("dialog.emptyUrl");
    }
    try {
      const parsed = new URL(trimmed);
      if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
        return $t("dialog.invalidUrl");
      }
      return null;
    } catch {
      return $t("dialog.invalidUrl");
    }
  }

  function handleSubmit() {
    errorMsg = validateUrl(urlInput);
    if (errorMsg) return;

    dispatch("submit", urlInput.trim());
    urlInput = "";
    errorMsg = null;
  }

  function handleCancel() {
    urlInput = "";
    errorMsg = null;
    dispatch("close");
  }

  function handleKeydown(e) {
    if (e.key === "Escape") {
      handleCancel();
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if open}
  <div class="dialog-backdrop" on:click={handleCancel} on:keydown={handleKeydown} role="presentation">
    <div
      class="dialog-card"
      on:click|stopPropagation
      on:keydown|stopPropagation
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-labelledby="dialog-title"
    >
      <div class="dialog-header">
        <h3 id="dialog-title" class="dialog-title">{$t("dialog.addTitle")}</h3>
        <button class="close-btn" on:click={handleCancel} aria-label={$t("dialog.cancel")}>
          <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
            <path fill-rule="evenodd" d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z" clip-rule="evenodd" />
          </svg>
        </button>
      </div>

      <form on:submit|preventDefault={handleSubmit} class="dialog-form">
        <div class="form-group">
          <label for="url-field" class="form-label">{$t("dialog.urlLabel")}</label>
          <input
            id="url-field"
            bind:this={inputElement}
            bind:value={urlInput}
            type="text"
            dir="ltr"
            placeholder={$t("dialog.urlPlaceholder")}
            class:input-error={errorMsg}
          />
          {#if errorMsg}
            <span class="error-text">{errorMsg}</span>
          {/if}
        </div>

        <div class="dialog-actions">
          <button type="button" class="btn btn-secondary" on:click={handleCancel}>
            {$t("dialog.cancel")}
          </button>
          <button type="submit" class="btn btn-primary">
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16">
              <path fill-rule="evenodd" d="M3 17a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm3.293-7.707a1 1 0 011.414 0L9 10.586V3a1 1 0 112 0v7.586l1.293-1.293a1 1 0 111.414 1.414l-3 3a1 1 0 01-1.414 0l-3-3a1 1 0 010-1.414z" clip-rule="evenodd" />
            </svg>
            <span>{$t("dialog.submit")}</span>
          </button>
        </div>
      </form>
    </div>
  </div>
{/if}

<style>
  .dialog-backdrop {
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
    justify-content: space-between;
  }

  .dialog-title {
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

  .dialog-form {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .form-label {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--color-text);
  }

  .input-error {
    border-color: var(--color-danger);
  }

  .error-text {
    font-size: 0.775rem;
    color: var(--color-danger);
  }

  .dialog-actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.75rem;
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

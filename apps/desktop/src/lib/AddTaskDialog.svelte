<script>
  import { t } from "./i18n.js";
  import { createEventDispatcher } from "svelte";

  export let open = false;
  export let initialUrl = "";

  let urlInput = "";
  let errorMsg = null;
  let inputElement;

  const dispatch = createEventDispatcher();

  let wasOpen = false;
  let prevInitialUrl = "";

  $: if (open) {
    const newlyOpened = !wasOpen;
    const urlChanged = initialUrl !== prevInitialUrl;
    if ((newlyOpened && initialUrl) || (urlChanged && initialUrl)) {
      urlInput = initialUrl;
      errorMsg = null;
    }
    if (newlyOpened || (urlChanged && initialUrl)) {
      setTimeout(() => {
        inputElement?.focus();
      }, 50);
    }
    wasOpen = true;
    prevInitialUrl = initialUrl;
  } else {
    wasOpen = false;
    prevInitialUrl = "";
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
          <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18" aria-hidden="true">
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
            class="form-control"
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
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
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
    background-color: rgba(7, 15, 46, 0.65);
    backdrop-filter: blur(4px);
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
    display: flex;
    align-items: center;
    justify-content: center;
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

  .form-control {
    font-family: inherit;
    font-size: 0.875rem;
    color: var(--color-text);
    background-color: var(--color-bg-alt);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: 0.55rem 0.85rem;
    transition: border-color var(--transition-normal), box-shadow var(--transition-normal);
  }

  .form-control:focus {
    outline: none;
    border-color: var(--color-border-focus);
    box-shadow: 0 0 0 2px var(--color-accent-light);
  }

  .input-error {
    border-color: var(--color-danger) !important;
  }

  .error-text {
    font-size: 0.775rem;
    color: var(--color-danger);
    font-weight: 500;
  }

  .dialog-actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.75rem;
  }

  .btn {
    padding: 0.5rem 1.15rem;
    font-weight: 600;
    border-radius: var(--radius-sm);
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
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
</style>

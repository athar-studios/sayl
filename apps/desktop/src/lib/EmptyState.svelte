<script>
  import { t } from "./i18n.js";
  import { createEventDispatcher } from "svelte";

  const dispatch = createEventDispatcher();
</script>

<div class="empty-state">
  <div class="empty-icon-wrap">
    <svg viewBox="0 0 64 64" fill="none" class="empty-svg" aria-hidden="true">
      <rect x="8" y="12" width="48" height="40" rx="8" stroke="currentColor" stroke-width="2.5" />
      <path d="M32 20v18m0 0l-7-7m7 7l7-7" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" />
      <path d="M22 44h20" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" />
    </svg>
  </div>
  <h2 class="empty-title">{$t("empty.title")}</h2>
  <p class="empty-desc">{$t("empty.description")}</p>
  <button class="empty-action-btn" on:click={() => dispatch("add")}>
    <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18" aria-hidden="true">
      <path fill-rule="evenodd" d="M10 3a1 1 0 011 1v5h5a1 1 0 110 2h-5v5a1 1 0 11-2 0v-5H4a1 1 0 110-2h5V4a1 1 0 011-1z" clip-rule="evenodd" />
    </svg>
    <span>{$t("empty.action")}</span>
  </button>
</div>

<style>
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 4rem 1.5rem;
    text-align: center;
    flex: 1;
  }

  .empty-icon-wrap {
    width: 5rem;
    height: 5rem;
    border-radius: var(--radius-lg);
    background-color: var(--color-bg-card);
    border: 1px dashed var(--color-border);
    display: flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 1.25rem;
    color: var(--color-accent);
  }

  .empty-svg {
    width: 2.75rem;
    height: 2.75rem;
  }

  .empty-title {
    font-size: 1.25rem;
    font-weight: 700;
    color: var(--color-text);
    margin-bottom: 0.5rem;
  }

  .empty-desc {
    font-size: 0.875rem;
    color: var(--color-text-muted);
    max-width: 26rem;
    margin-bottom: 1.5rem;
    line-height: 1.6;
  }

  .empty-action-btn {
    background-color: var(--color-accent);
    color: var(--color-accent-text);
    padding: 0.65rem 1.25rem;
    font-size: 0.925rem;
    font-weight: 600;
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-sm);
  }

  .empty-action-btn:hover {
    background-color: var(--color-accent-hover);
  }
</style>

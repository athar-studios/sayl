<script>
  import { t } from "./i18n.js";
  import { createEventDispatcher } from "svelte";
  import logoFullRaw from "../assets/logo-full.svg?raw";
  import logoWhiteRaw from "../assets/logo-white.svg?raw";

  const dispatch = createEventDispatcher();
</script>

<div class="empty-state">
  <div class="empty-logo-wrap" aria-hidden="true">
    <div class="empty-logo-svg logo-light">
      {@html logoFullRaw}
    </div>
    <div class="empty-logo-svg logo-dark">
      {@html logoWhiteRaw}
    </div>
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

  .empty-logo-wrap {
    width: 5rem;
    height: 5rem;
    display: flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 1.5rem;
  }

  .empty-logo-svg {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    filter: drop-shadow(0 4px 12px rgba(0, 182, 189, 0.25));
  }

  .empty-logo-svg :global(svg) {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .logo-dark {
    display: none;
  }
  .logo-light {
    display: flex;
  }

  :global([data-theme="dark"]) .logo-dark {
    display: flex !important;
  }
  :global([data-theme="dark"]) .logo-light {
    display: none !important;
  }

  @media (prefers-color-scheme: dark) {
    :global(:root:not([data-theme="light"])) .logo-dark {
      display: flex;
    }
    :global(:root:not([data-theme="light"])) .logo-light {
      display: none;
    }
  }

  .empty-title {
    font-size: 1.25rem;
    font-weight: 700;
    color: var(--color-text);
    margin-bottom: 0.5rem;
  }

  .empty-desc {
    font-size: 0.9rem;
    color: var(--color-text-muted);
    max-width: 24rem;
    margin-bottom: 1.75rem;
    line-height: 1.6;
  }

  .empty-action-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    background: linear-gradient(135deg, #00b6bd, #0b1a4e) !important;
    color: #ffffff !important;
    padding: 0.65rem 1.35rem;
    font-size: 0.925rem;
    font-weight: 600;
    border-radius: var(--radius-md);
    border: none;
    cursor: pointer;
    box-shadow: 0 4px 14px rgba(0, 182, 189, 0.35);
    transition: transform 120ms ease, box-shadow 120ms ease;
  }

  .empty-action-btn:hover {
    transform: translateY(-1px);
    box-shadow: 0 6px 18px rgba(0, 182, 189, 0.45);
  }

  .empty-action-btn:active {
    transform: translateY(0);
  }
</style>
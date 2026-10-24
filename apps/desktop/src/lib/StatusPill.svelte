<script>
  import { t } from "./i18n.js";

  export let state = "Queued";

  const stateClassMap = {
    Queued: "pill-queued",
    Probing: "pill-probing",
    Downloading: "pill-downloading",
    Verifying: "pill-verifying",
    Finalizing: "pill-finalizing",
    Completed: "pill-completed",
    Pausing: "pill-pausing",
    Paused: "pill-paused",
    Cancelling: "pill-cancelling",
    Cancelled: "pill-cancelled",
    Failed: "pill-failed",
  };

  $: pillClass = stateClassMap[state] || "pill-queued";
  $: label = $t("state." + state);
</script>

<span class="status-pill {pillClass}">
  <span class="status-dot"></span>
  <span class="status-text">{label}</span>
</span>

<style>
  .status-pill {
    display: inline-flex;
    align-items: center;
    gap: 0.375rem;
    padding: 0.2rem 0.6rem;
    border-radius: var(--radius-full);
    font-size: 0.75rem;
    font-weight: 600;
    line-height: 1;
    white-space: nowrap;
  }

  .status-dot {
    width: 0.45rem;
    height: 0.45rem;
    border-radius: 50%;
    background-color: currentColor;
  }

  .pill-downloading {
    background-color: var(--color-accent-light);
    color: var(--color-accent);
  }
  .pill-downloading .status-dot {
    animation: pulse 1.5s infinite;
  }

  .pill-probing, .pill-verifying, .pill-finalizing {
    background-color: rgba(2, 132, 199, 0.15);
    color: var(--color-info);
  }

  .pill-completed {
    background-color: rgba(22, 163, 74, 0.15);
    color: var(--color-success);
  }

  .pill-paused, .pill-pausing {
    background-color: rgba(217, 119, 6, 0.15);
    color: var(--color-warning);
  }

  .pill-failed {
    background-color: var(--color-danger-light);
    color: var(--color-danger);
  }

  .pill-queued, .pill-cancelling, .pill-cancelled {
    background-color: var(--color-bg-alt);
    color: var(--color-text-muted);
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.3; }
  }
</style>

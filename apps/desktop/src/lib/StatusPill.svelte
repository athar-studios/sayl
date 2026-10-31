<script>
  import { t } from "./i18n.js";

  export let state = "Queued";
  export let isConnecting = false;

  const stateClassMap = {
    // Running / active: turquoise #00b6bd
    Probing: "pill-running",
    Downloading: "pill-running",
    Verifying: "pill-running",
    Finalizing: "pill-running",

    // Completed: green
    Completed: "pill-completed",

    // Paused: yellow / amber
    Pausing: "pill-paused",
    Paused: "pill-paused",

    // Failed: red
    Failed: "pill-failed",

    // Waiting / queued: gray
    Queued: "pill-queued",
    Cancelling: "pill-queued",
    Cancelled: "pill-queued",
  };

  $: pillClass = isConnecting ? "pill-running" : (stateClassMap[state] || "pill-queued");
  $: label = isConnecting ? $t("task.connecting") : $t("state." + state);
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

  /* Fixed state colours (Req 5) */
  .pill-running {
    background-color: var(--color-accent-light);
    color: var(--state-running, #00b6bd);
  }
  .pill-running .status-dot {
    animation: pulse 1.5s infinite;
  }

  .pill-completed {
    background-color: rgba(22, 163, 74, 0.15);
    color: var(--state-completed, #16a34a);
  }

  .pill-paused {
    background-color: rgba(217, 119, 6, 0.15);
    color: var(--state-paused, #d97706);
  }

  .pill-failed {
    background-color: var(--color-danger-light);
    color: var(--state-failed, #dc2626);
  }

  .pill-queued {
    background-color: var(--color-bg-alt);
    color: var(--state-queued, #64748b);
  }

  @keyframes pulse {
    0%, 100% {
      opacity: 1;
    }
    50% {
      opacity: 0.3;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .pill-running .status-dot {
      animation: none !important;
    }
  }
</style>

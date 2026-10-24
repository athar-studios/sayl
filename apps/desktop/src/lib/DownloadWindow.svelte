<script>
  import { t, currentLang } from "./i18n.js";
  import {
    formatBytes,
    formatSpeed,
    formatEta,
    formatPercent,
    canPause,
    canResume,
    canCancel,
  } from "./format.js";
  import StatusPill from "./StatusPill.svelte";
  import { createEventDispatcher } from "svelte";
  import logoFull from "../assets/logo-full.svg";
  import logoWhite from "../assets/logo-white.svg";

  export let task;
  export let autoShutdown = false;

  const dispatch = createEventDispatcher();

  $: isCompleted = task.state === "Completed";
  $: isDownloading = task.state === "Downloading";
  $: allowPause = canPause(task.state);
  $: allowResume = canResume(task.state);
  $: allowCancel = canCancel(task.state);

  $: percentNum =
    task.total_size && task.total_size > 0
      ? Math.min(100, Math.max(0, Math.round((task.written / task.total_size) * 1000) / 10))
      : null;

  $: writtenFormatted = formatBytes(task.written, $currentLang);
  $: totalFormatted = task.total_size
    ? formatBytes(task.total_size, $currentLang)
    : $t("task.unknownSize");
  $: speedFormatted = formatSpeed(task.speed_bps, $currentLang);
  $: etaFormatted = formatEta(task.eta_secs, $currentLang);
  $: percentFormatted = percentNum !== null ? formatPercent(percentNum, $currentLang) : null;
</script>

<div class="download-window" role="dialog" aria-label="{task.filename || $t('downloadWindow.title')}">
  <!-- Window Header -->
  <div class="window-header">
    <div class="brand-wrap">
      <div class="logo-box" aria-hidden="true">
        <img src={logoFull} alt="SAYL" class="logo-img logo-light" />
        <img src={logoWhite} alt="SAYL" class="logo-img logo-dark" />
      </div>
      <span class="file-name" title={task.filename}><bdi>{task.filename || "—"}</bdi></span>
    </div>

    <div class="header-end">
      <StatusPill state={task.state} />
      <button
        class="close-btn"
        title={$t("downloadWindow.close")}
        aria-label={$t("downloadWindow.close")}
        on:click={() => dispatch("close", task.id)}
      >
        <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
          <path
            fill-rule="evenodd"
            d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z"
            clip-rule="evenodd"
          />
        </svg>
      </button>
    </div>
  </div>

  <!-- Progress Bar (Teal-to-Navy gradient) -->
  <div class="progress-section">
    <div class="progress-bar-wrap">
      <div
        class="progress-bar-fill"
        class:completed={isCompleted}
        style="width: {percentNum !== null ? percentNum : (isCompleted ? 100 : 0)}%;"
      />
    </div>

    <div class="progress-labels">
      <span class="size-text"><bdi>{writtenFormatted}</bdi> / <bdi>{totalFormatted}</bdi></span>
      {#if percentFormatted}
        <span class="percent-text"><bdi>{percentFormatted}</bdi></span>
      {/if}
    </div>
  </div>

  <!-- Metrics Grid -->
  <div class="metrics-grid">
    <div class="metric-col">
      <span class="metric-label">{$t("task.speed")}</span>
      <span class="metric-val">
        {#if isDownloading}
          <bdi>{speedFormatted}</bdi>
        {:else}
          <bdi>—</bdi>
        {/if}
      </span>
    </div>

    <div class="metric-col">
      <span class="metric-label">{$t("task.eta")}</span>
      <span class="metric-val">
        {#if isDownloading}
          <bdi>{etaFormatted}</bdi>
        {:else}
          <bdi>—</bdi>
        {/if}
      </span>
    </div>

    <div class="metric-col">
      <span class="metric-label">{$t("downloadWindow.parts")}</span>
      <span class="metric-val">
        {#if task.parts !== null && task.parts !== undefined}
          <bdi>{task.parts}</bdi>
        {:else}
          <bdi>—</bdi>
        {/if}
      </span>
    </div>
  </div>

  <!-- Action Controls -->
  <div class="window-actions">
    {#if isCompleted}
      <button
        class="btn btn-primary action-btn"
        on:click={() => dispatch("openFile", task.id)}
      >
        <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15" aria-hidden="true">
          <path d="M10 12a2 2 0 100-4 2 2 0 000 4z" />
          <path fill-rule="evenodd" d="M.458 10C1.732 5.943 5.522 3 10 3s8.268 2.943 9.542 7c-1.274 4.057-5.064 7-9.542 7S1.732 14.057.458 10zM14 10a4 4 0 11-8 0 4 4 0 018 0z" clip-rule="evenodd" />
        </svg>
        <span>{$t("downloadWindow.openFile")}</span>
      </button>

      <button
        class="btn btn-secondary action-btn"
        on:click={() => dispatch("reveal", task.id)}
      >
        <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15" aria-hidden="true">
          <path fill-rule="evenodd" d="M2 6a2 2 0 012-2h4l2 2h6a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" clip-rule="evenodd" />
        </svg>
        <span>{$t("downloadWindow.revealFolder")}</span>
      </button>
    {:else}
      <div class="active-controls">
        {#if allowPause}
          <button
            class="control-btn"
            title={$t("task.pause")}
            aria-label={$t("task.pause")}
            on:click={() => dispatch("pause", task.id)}
          >
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
              <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zM7 8a1 1 0 012 0v4a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v4a1 1 0 102 0V8a1 1 0 00-1-1z" clip-rule="evenodd" />
            </svg>
            <span>{$t("task.pause")}</span>
          </button>
        {/if}

        {#if allowResume}
          <button
            class="control-btn control-btn-accent"
            title={$t("task.resume")}
            aria-label={$t("task.resume")}
            on:click={() => dispatch("resume", task.id)}
          >
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
              <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM9.555 7.168A1 1 0 008 8v4a1 1 0 001.555.832l3-2a1 1 0 000-1.664l-3-2z" clip-rule="evenodd" />
            </svg>
            <span>{$t("task.resume")}</span>
          </button>
        {/if}

        {#if allowCancel}
          <button
            class="control-btn control-btn-danger"
            title={$t("task.cancel")}
            aria-label={$t("task.cancel")}
            on:click={() => dispatch("cancel", task.id)}
          >
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
              <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clip-rule="evenodd" />
            </svg>
            <span>{$t("task.cancel")}</span>
          </button>
        {/if}
      </div>
    {/if}
  </div>

  <!-- Shutdown on Completion Toggle -->
  <div class="shutdown-row">
    <label class="shutdown-toggle-label">
      <input
        type="checkbox"
        class="toggle-checkbox"
        checked={autoShutdown}
        on:change={(e) => dispatch("toggleAutoShutdown", e.target.checked)}
      />
      <span class="toggle-text">{$t("shutdown.toggle")}</span>
    </label>
  </div>
</div>

<style>
  .download-window {
    width: 330px;
    background-color: var(--color-bg-card);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    padding: 0.85rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    pointer-events: auto;
    animation: window-in 220ms ease;
  }

  .window-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .brand-wrap {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    overflow: hidden;
    flex: 1;
  }

  .logo-box {
    width: 1.35rem;
    height: 1.35rem;
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

  .logo-dark {
    display: none;
  }
  .logo-light {
    display: block;
  }

  :global([data-theme="dark"]) .logo-dark {
    display: block !important;
  }
  :global([data-theme="dark"]) .logo-light {
    display: none !important;
  }

  @media (prefers-color-scheme: dark) {
    :global(:root:not([data-theme="light"])) .logo-dark {
      display: block;
    }
    :global(:root:not([data-theme="light"])) .logo-light {
      display: none;
    }
  }

  .file-name {
    font-size: 0.85rem;
    font-weight: 700;
    color: var(--color-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .header-end {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    flex-shrink: 0;
  }

  .close-btn {
    width: 1.5rem;
    height: 1.5rem;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-muted);
    background: transparent;
    border-radius: var(--radius-sm);
  }

  .close-btn:hover {
    background-color: var(--color-bg-alt);
    color: var(--color-text);
  }

  .progress-section {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .progress-bar-wrap {
    height: 6px;
    background-color: var(--color-bg-alt);
    border-radius: var(--radius-full);
    overflow: hidden;
  }

  .progress-bar-fill {
    height: 100%;
    background: var(--color-accent-gradient);
    border-radius: var(--radius-full);
    transition: width 200ms ease;
  }

  .progress-bar-fill.completed {
    background: #16a34a !important;
  }

  .progress-labels {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 0.725rem;
    color: var(--color-text-muted);
  }

  .percent-text {
    font-weight: 700;
    color: var(--color-accent);
  }

  .metrics-grid {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: 0.35rem;
    padding: 0.45rem 0.5rem;
    background-color: var(--color-bg-alt);
    border-radius: var(--radius-sm);
  }

  .metric-col {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.1rem;
    text-align: center;
  }

  .metric-label {
    font-size: 0.675rem;
    color: var(--color-text-subtle);
  }

  .metric-val {
    font-size: 0.75rem;
    font-weight: 700;
    color: var(--color-text);
    white-space: nowrap;
  }

  .window-actions {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .active-controls {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    width: 100%;
  }

  .control-btn {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.25rem;
    padding: 0.35rem 0.5rem;
    font-size: 0.775rem;
    font-weight: 600;
    border-radius: var(--radius-sm);
    background-color: var(--color-bg-alt);
    color: var(--color-text-muted);
    transition: background-color var(--transition-normal), color var(--transition-normal);
  }

  .control-btn:hover {
    background-color: var(--color-bg-card-hover);
    color: var(--color-text);
  }

  .control-btn-accent {
    color: var(--color-accent);
  }

  .control-btn-accent:hover {
    background-color: var(--color-accent-light);
  }

  .control-btn-danger:hover {
    background-color: var(--color-danger-light);
    color: var(--color-danger);
  }

  .action-btn {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.35rem;
    padding: 0.4rem 0.6rem;
    font-size: 0.8rem;
    font-weight: 600;
    border-radius: var(--radius-sm);
  }

  .btn-primary {
    background: var(--color-accent-gradient);
    color: #ffffff;
  }

  .btn-secondary {
    background-color: var(--color-bg-alt);
    border: 1px solid var(--color-border);
    color: var(--color-text);
  }

  .shutdown-row {
    padding-top: 0.4rem;
    border-top: 1px solid var(--color-border);
  }

  .shutdown-toggle-label {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    font-size: 0.725rem;
    color: var(--color-text-muted);
    cursor: pointer;
    user-select: none;
  }

  .toggle-checkbox {
    cursor: pointer;
    accent-color: var(--color-accent);
  }

  @keyframes window-in {
    from {
      opacity: 0;
      transform: translateY(10px) scale(0.96);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .download-window {
      animation: none !important;
    }
    .progress-bar-fill {
      transition: none !important;
    }
  }
</style>

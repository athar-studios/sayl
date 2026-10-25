<script>
  import { t, currentLang } from "./i18n.js";
  import {
    formatBytes,
    formatSpeed,
    formatEta,
    formatPercent,
    canPauseTask,
    canResumeTask,
    canCancel,
  } from "./format.js";
  import { friendlyError } from "./errors.js";
  import StatusPill from "./StatusPill.svelte";
  import { createEventDispatcher } from "svelte";
  import iconSvg from "../assets/icon.svg";

  export let task;
  export let autoShutdown = false;
  export let autoClose = false;
  export let isNativeWindow = false;

  const dispatch = createEventDispatcher();

  $: isCompleted = task.state === "Completed";
  $: isDownloading = task.state === "Downloading";
  $: isFailed = task.state === "Failed";
  $: allowPause = canPauseTask(task.state);
  $: allowResume = canResumeTask(task.state);
  $: allowCancel = canCancel(task.state);
  $: errInfo = isFailed ? friendlyError(task, $currentLang) : null;

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
  $: isRtl = $currentLang === "ar";
</script>

<div
  class="download-window"
  class:rtl={isRtl}
  dir={isRtl ? "rtl" : "ltr"}
  role="dialog"
  aria-label="{task.filename || $t('downloadWindow.title')}"
>
  <!-- Window Header -->
  <div class="window-header">
    <div class="brand-wrap">
      <div class="logo-box" aria-hidden="true">
        <img src={iconSvg} alt="SAYL" class="logo-img" />
      </div>
      <span class="file-name" title={task.filename}>
        <bdi dir="auto">{task.filename || "—"}</bdi>
      </span>
    </div>

    <div class="header-end">
      <StatusPill state={task.state} />
      {#if !isNativeWindow}
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
      {/if}
    </div>
  </div>

  <!-- 16px Turquoise Progress Bar -->
  <div class="progress-section">
    <div class="progress-bar-wrap" role="progressbar" aria-valuenow={percentNum ?? 0} aria-valuemin="0" aria-valuemax="100">
      <div
        class="progress-bar-fill"
        class:completed={isCompleted}
        style="width: {percentNum !== null ? percentNum : (isCompleted ? 100 : 0)}%;"
      ></div>
    </div>
  </div>

  <!-- 2-Column Grid -->
  <div class="labels-grid">
    <!-- Column 1: Progress percentage & Status, Downloaded / Total size -->
    <div class="grid-col">
      <div class="grid-row">
        <span class="grid-label">{$t("downloadWindow.progress")}:</span>
        <span class="grid-value">
          <bdi dir="ltr">{percentFormatted || (isCompleted ? formatPercent(100, $currentLang) : "0%")}</bdi>
        </span>
      </div>
      <div class="grid-row">
        <span class="grid-label">{$t("downloadWindow.size") || $t("table.size")}:</span>
        <span class="grid-value">
          <bdi dir="ltr">{writtenFormatted} / {totalFormatted}</bdi>
        </span>
      </div>
    </div>

    <!-- Column 2: Current speed, ETA / Remaining time -->
    <div class="grid-col">
      <div class="grid-row">
        <span class="grid-label">{$t("downloadWindow.speed")}:</span>
        <span class="grid-value">
          <bdi dir="ltr">{isDownloading ? speedFormatted : "—"}</bdi>
        </span>
      </div>
      <div class="grid-row">
        <span class="grid-label">{$t("downloadWindow.eta")}:</span>
        <span class="grid-value">
          <bdi dir="ltr">{isDownloading ? etaFormatted : "—"}</bdi>
        </span>
      </div>
    </div>
  </div>

  <!-- Real Error Display (when Failed) -->
  {#if isFailed}
    <div class="error-banner" role="alert">
      <svg class="error-icon" viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
        <path
          fill-rule="evenodd"
          d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z"
          clip-rule="evenodd"
        />
      </svg>
      <span class="error-msg" title={task.error || errInfo?.hint || ""}>
        <bdi dir="auto">{task.error || errInfo?.hint || $t("status.failed")}</bdi>
      </span>
    </div>
  {/if}

  <!-- Action Controls -->
  <div class="window-footer">
    <div class="window-actions">
      {#if isCompleted}
        <button
          class="action-btn btn-primary"
          on:click={() => dispatch("openFile", task.id)}
        >
          <span>{$t("downloadWindow.openFile")}</span>
        </button>
        <button
          class="action-btn btn-secondary"
          on:click={() => dispatch("reveal", task.id)}
        >
          <span>{$t("downloadWindow.revealFolder")}</span>
        </button>
      {:else}
        {#if allowPause}
          <button
            class="action-btn btn-secondary"
            title={$t("toolbar.pause")}
            on:click={() => dispatch("pause", task.id)}
          >
            <span>{$t("toolbar.pause")}</span>
          </button>
        {/if}

        {#if allowResume && !isFailed}
          <button
            class="action-btn btn-primary"
            title={$t("toolbar.resume")}
            on:click={() => dispatch("resume", task.id)}
          >
            <span>{$t("toolbar.resume")}</span>
          </button>
        {/if}

        {#if isFailed}
          <button
            class="action-btn btn-primary"
            title={$t("downloadWindow.retry")}
            on:click={() => dispatch("retry", task.id)}
          >
            <span>{$t("downloadWindow.retry")}</span>
          </button>
        {/if}

        {#if allowCancel}
          <button
            class="action-btn btn-danger"
            title={$t("task.cancel")}
            on:click={() => dispatch("cancel", task.id)}
          >
            <span>{$t("task.cancel")}</span>
          </button>
        {/if}
      {/if}
    </div>

    <!-- Options -->
    <div class="options-row">
      <label class="option-label">
        <input
          type="checkbox"
          class="option-checkbox"
          checked={autoClose}
          on:change={(e) => dispatch("toggleAutoClose", e.target.checked)}
        />
        <span>{$t("downloadWindow.autoClose")}</span>
      </label>
      <label class="option-label">
        <input
          type="checkbox"
          class="option-checkbox"
          checked={autoShutdown}
          on:change={(e) => dispatch("toggleAutoShutdown", e.target.checked)}
        />
        <span>{$t("shutdown.toggle")}</span>
      </label>
    </div>
  </div>
</div>

<style>
  .download-window {
    width: 460px;
    height: 260px;
    max-width: 460px;
    max-height: 260px;
    box-sizing: border-box;
    padding: 1rem;
    background: var(--color-bg-card, #ffffff);
    color: var(--color-text, #1f2937);
    border: 1px solid var(--color-border, #d4e0ee);
    border-radius: 8px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    font-family: var(--font-family, "Segoe UI", Arial, sans-serif);
    user-select: none;
    overflow: hidden;
  }

  .download-window.rtl {
    text-align: right;
  }

  :global([data-theme="dark"]) .download-window {
    background: #1f2937;
    color: #f3f4f6;
    border-color: #374151;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
  }

  /* Window Header */
  .window-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    height: 28px;
    flex-shrink: 0;
  }

  .brand-wrap {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    overflow: hidden;
    flex: 1;
    min-width: 0;
  }

  .logo-box {
    width: 20px;
    height: 20px;
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

  .file-name {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--color-text, #1f2937);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  :global([data-theme="dark"]) .file-name {
    color: #f3f4f6;
  }

  .header-end {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }

  .close-btn {
    width: 24px;
    height: 24px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-muted, #4b5563);
    background: transparent;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    transition: background-color 120ms ease, color 120ms ease;
  }

  .close-btn:hover {
    background: var(--color-bg-card-hover, #f0f4fa);
    color: var(--color-text, #1f2937);
  }

  :global([data-theme="dark"]) .close-btn {
    color: #9ca3af;
  }

  :global([data-theme="dark"]) .close-btn:hover {
    background: #374151;
    color: #f3f4f6;
  }

  /* 16px Turquoise Progress Bar */
  .progress-section {
    width: 100%;
    flex-shrink: 0;
  }

  .progress-bar-wrap {
    height: 16px;
    width: 100%;
    background-color: #e5e7eb;
    border-radius: 8px;
    overflow: hidden;
    box-sizing: border-box;
  }

  :global([data-theme="dark"]) .progress-bar-wrap {
    background-color: #1f2937;
    border: 1px solid #374151;
  }

  .progress-bar-fill {
    height: 100%;
    background-color: #00b6bd;
    border-radius: 8px;
    transition: width 150ms ease;
  }

  .progress-bar-fill.completed {
    background-color: #16a34a;
  }

  /* 2-Column Grid */
  .labels-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px 16px;
    flex-shrink: 0;
  }

  .grid-col {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  .grid-row {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.8125rem;
    line-height: 1.4;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .grid-label {
    color: #4b5563;
    flex-shrink: 0;
  }

  :global([data-theme="dark"]) .grid-label {
    color: #d1d5db;
  }

  .grid-value {
    font-weight: 600;
    color: #1f2937;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  :global([data-theme="dark"]) .grid-value {
    color: #f3f4f6;
  }

  /* Real Error Banner */
  .error-banner {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    background-color: #fee2e2;
    border: 1px solid #f87171;
    border-radius: 6px;
    color: #dc2626;
    font-size: 0.75rem;
    line-height: 1.3;
    overflow: hidden;
    flex-shrink: 0;
  }

  :global([data-theme="dark"]) .error-banner {
    background-color: rgba(239, 68, 68, 0.15);
    border-color: #ef4444;
    color: #f87171;
  }

  .error-icon {
    flex-shrink: 0;
  }

  .error-msg {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 500;
    min-width: 0;
  }

  /* Footer & Action Controls */
  .window-footer {
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex-shrink: 0;
  }

  .window-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
  }

  .action-btn {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 32px;
    padding: 0 12px;
    font-size: 0.8125rem;
    font-weight: 600;
    border-radius: 6px;
    cursor: pointer;
    transition: all 120ms ease;
    border: 1px solid transparent;
  }

  .btn-primary {
    background-color: #00b6bd;
    color: #ffffff;
    border-color: #009ea4;
  }

  .btn-primary:hover {
    background-color: #009ea4;
  }

  .btn-secondary {
    background-color: #f3f4f6;
    color: #1f2937;
    border-color: #d1d5db;
  }

  .btn-secondary:hover {
    background-color: #e5e7eb;
  }

  :global([data-theme="dark"]) .btn-secondary {
    background-color: #374151;
    color: #f3f4f6;
    border-color: #4b5563;
  }

  :global([data-theme="dark"]) .btn-secondary:hover {
    background-color: #4b5563;
  }

  .btn-danger {
    background-color: #fee2e2;
    color: #dc2626;
    border-color: #fca5a5;
  }

  .btn-danger:hover {
    background-color: #fecaca;
  }

  :global([data-theme="dark"]) .btn-danger {
    background-color: rgba(239, 68, 68, 0.2);
    color: #f87171;
    border-color: #ef4444;
  }

  .options-row {
    display: flex;
    align-items: center;
    justify-content: flex-end;
  }

  .download-window.rtl .options-row {
    justify-content: flex-start;
  }

  .option-label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 0.75rem;
    color: #4b5563;
    cursor: pointer;
  }

  :global([data-theme="dark"]) .option-label {
    color: #9ca3af;
  }

  .option-checkbox {
    cursor: pointer;
    accent-color: #00b6bd;
  }

  @media (prefers-reduced-motion: reduce) {
    .progress-bar-fill {
      transition: none !important;
    }
  }
</style>
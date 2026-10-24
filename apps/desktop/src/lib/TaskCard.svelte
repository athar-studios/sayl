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
  import { classifyFile } from "./classify.js";
  import StatusPill from "./StatusPill.svelte";
  import { createEventDispatcher } from "svelte";

  export let task;
  export let selected = false;

  const dispatch = createEventDispatcher();

  let confirmingCancel = false;
  let confirmTimeout = null;

  function handleCancelClick() {
    if (confirmingCancel) {
      if (confirmTimeout) clearTimeout(confirmTimeout);
      confirmingCancel = false;
      dispatch("cancel", task.id);
    } else {
      confirmingCancel = true;
      confirmTimeout = setTimeout(() => {
        confirmingCancel = false;
      }, 3500);
    }
  }

  $: fileCategory = classifyFile(task.filename, task.mime);
  $: writtenFormatted = formatBytes(task.written, $currentLang);
  $: totalFormatted = task.total_size ? formatBytes(task.total_size, $currentLang) : $t("task.unknownSize");
  $: speedFormatted = formatSpeed(task.speed_bps, $currentLang);
  $: etaFormatted = formatEta(task.eta_secs, $currentLang);

  $: percentNum = task.total_size && task.total_size > 0
    ? Math.min(100, Math.max(0, Math.round((task.written / task.total_size) * 1000) / 10))
    : null;

  $: percentFormatted = percentNum !== null ? formatPercent(percentNum, $currentLang) : null;

  $: allowPause = canPause(task.state);
  $: allowResume = canResume(task.state);
  $: allowCancel = canCancel(task.state);
  $: isCompleted = task.state === "Completed";
  $: isDownloading = task.state === "Downloading";
</script>

<div
  class="task-card"
  class:selected
  on:click={() => dispatch("select", task.id)}
  on:keydown={(e) => { if (e.key === 'Enter') dispatch('select', task.id); }}
  role="button"
  tabindex="0"
  aria-label="{task.filename || 'Task'}"
>
  <div class="card-header">
    <div class="file-info">
      <div class="file-icon file-icon-{fileCategory}" aria-hidden="true">
        {#if fileCategory === 'video'}
          <!-- Video Icon -->
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="22" height="22">
            <rect x="2" y="4" width="20" height="16" rx="3" />
            <polygon points="10 8 16 12 10 16 10 8" fill="currentColor" stroke="none" />
          </svg>
        {:else if fileCategory === 'audio'}
          <!-- Audio Icon -->
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="22" height="22">
            <path d="M9 18V5l12-2v13" />
            <circle cx="6" cy="18" r="3" fill="currentColor" />
            <circle cx="18" cy="16" r="3" fill="currentColor" />
          </svg>
        {:else if fileCategory === 'archive' || fileCategory === 'compressed'}
          <!-- Archive Icon -->
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="22" height="22">
            <path d="M4 4h16v4H4z" />
            <path d="M4 8v12a2 2 0 002 2h12a2 2 0 002-2V8" />
            <path d="M10 12h4m-2-2v4" />
          </svg>
        {:else if fileCategory === 'programs'}
          <!-- Program Icon -->
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="22" height="22">
            <rect x="3" y="3" width="18" height="18" rx="2" />
            <path d="M3 9h18" />
            <path d="M9 21V9" />
          </svg>
        {:else if fileCategory === 'document' || fileCategory === 'documents'}
          <!-- Document Icon -->
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="22" height="22">
            <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
            <polyline points="14 2 14 8 20 8" />
            <line x1="16" y1="13" x2="8" y2="13" />
            <line x1="16" y1="17" x2="8" y2="17" />
          </svg>
        {:else}
          <!-- Generic File Icon -->
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="22" height="22">
            <path d="M13 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V9z" />
            <polyline points="13 2 13 9 20 9" />
          </svg>
        {/if}
      </div>

      <div class="file-details">
        <span class="file-name" title={task.filename}><bdi>{task.filename || "—"}</bdi></span>
        <span class="file-url" title={task.url} dir="ltr">{task.url}</span>
      </div>
    </div>

    <div class="card-status-actions">
      <StatusPill state={task.state} />

      <div class="action-buttons">
        {#if isCompleted}
          <button
            class="btn-open-file"
            title={$t("task.openFile")}
            aria-label="{$t('task.openFile')}: {task.filename}"
            on:click|stopPropagation={() => dispatch("reveal", task.id)}
          >
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
              <path fill-rule="evenodd" d="M2 6a2 2 0 012-2h4l2 2h6a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" clip-rule="evenodd" />
            </svg>
            <span>{$t("task.openFile")}</span>
          </button>
        {/if}

        {#if allowPause}
          <button
            class="icon-btn"
            title={$t("task.pause")}
            aria-label="{$t('task.pause')}: {task.filename}"
            on:click|stopPropagation={() => dispatch("pause", task.id)}
          >
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
              <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zM7 8a1 1 0 012 0v4a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v4a1 1 0 102 0V8a1 1 0 00-1-1z" clip-rule="evenodd" />
            </svg>
          </button>
        {/if}

        {#if allowResume}
          <button
            class="icon-btn icon-btn-accent"
            title={$t("task.resume")}
            aria-label="{$t('task.resume')}: {task.filename}"
            on:click|stopPropagation={() => dispatch("resume", task.id)}
          >
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
              <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM9.555 7.168A1 1 0 008 8v4a1 1 0 001.555.832l3-2a1 1 0 000-1.664l-3-2z" clip-rule="evenodd" />
            </svg>
          </button>
        {/if}

        {#if allowCancel}
          <button
            class="icon-btn icon-btn-danger"
            class:confirming={confirmingCancel}
            title={confirmingCancel ? $t("task.confirmCancel") : $t("task.cancel")}
            aria-label="{confirmingCancel ? $t('task.confirmCancel') : $t('task.cancel')}: {task.filename}"
            on:click|stopPropagation={handleCancelClick}
          >
            {#if confirmingCancel}
              <span class="confirm-text">{$t("task.confirmCancel")}</span>
            {:else}
              <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
                <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clip-rule="evenodd" />
              </svg>
            {/if}
          </button>
        {/if}

        {#if !isCompleted}
          <button
            class="icon-btn"
            title={$t("task.revealFolder")}
            aria-label="{$t('task.revealFolder')}: {task.filename}"
            on:click|stopPropagation={() => dispatch("reveal", task.id)}
          >
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
              <path fill-rule="evenodd" d="M2 6a2 2 0 012-2h4l2 2h6a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" clip-rule="evenodd" />
            </svg>
          </button>
        {/if}
      </div>
    </div>
  </div>

  <div class="progress-bar-wrap">
    <div
      class="progress-bar-fill"
      class:downloading={isDownloading}
      class:completed={isCompleted}
      style:width="{percentNum !== null ? percentNum : (isCompleted ? 100 : 0)}%"
    ></div>
  </div>

  <div class="card-footer">
    <div class="metrics">
      <span class="size-metric">
        <bdi>{writtenFormatted}</bdi> / <bdi>{totalFormatted}</bdi>
        {#if percentFormatted}
          <span class="percent-badge"><bdi>{percentFormatted}</bdi></span>
        {/if}
      </span>

      {#if isDownloading}
        <span class="dot-separator">•</span>
        <span class="speed-metric"><bdi>{speedFormatted}</bdi></span>
        <span class="dot-separator">•</span>
        <span class="eta-metric"><bdi>{etaFormatted}</bdi></span>
      {/if}
    </div>

    {#if task.error}
      <div class="error-msg" title={task.error}>
        <svg viewBox="0 0 20 20" fill="currentColor" width="14" height="14" aria-hidden="true">
          <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clip-rule="evenodd" />
        </svg>
        <span>{task.error}</span>
      </div>
    {/if}
  </div>
</div>

<style>
  @keyframes card-in {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .task-card {
    background-color: var(--color-bg-card);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: 1rem 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    box-shadow: var(--shadow-sm);
    transition: border-color var(--transition-normal), box-shadow var(--transition-normal);
    cursor: pointer;
    animation: card-in 220ms cubic-bezier(0.16, 1, 0.3, 1) backwards;
  }

  .task-card:hover {
    border-color: var(--color-border-focus);
    box-shadow: var(--shadow-md);
  }

  .task-card.selected {
    border-color: var(--color-accent);
    outline: 2px solid var(--color-accent-light);
  }

  .card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  .file-info {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    min-width: 0;
    flex: 1;
  }

  .file-icon {
    width: 2.5rem;
    height: 2.5rem;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .file-icon-video {
    background-color: rgba(168, 85, 247, 0.15);
    color: #a855f7;
  }

  .file-icon-audio {
    background-color: rgba(236, 72, 153, 0.15);
    color: #ec4899;
  }

  .file-icon-archive {
    background-color: rgba(245, 158, 11, 0.15);
    color: #f59e0b;
  }

  .file-icon-image {
    background-color: rgba(34, 197, 94, 0.15);
    color: #22c55e;
  }

  .file-icon-document {
    background-color: rgba(59, 130, 246, 0.15);
    color: #3b82f6;
  }

  .file-icon-generic {
    background-color: rgba(0, 182, 189, 0.15);
    color: #00b6bd;
  }

  .file-details {
    display: flex;
    flex-direction: column;
    min-width: 0;
    gap: 0.15rem;
  }

  .file-name {
    font-size: 0.95rem;
    font-weight: 700;
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
    text-align: start;
  }

  .card-status-actions {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-shrink: 0;
  }

  .action-buttons {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  .btn-open-file {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.35rem 0.75rem;
    background-color: rgba(0, 182, 189, 0.15);
    color: var(--color-accent);
    border: 1px solid rgba(0, 182, 189, 0.3);
    border-radius: var(--radius-sm);
    font-size: 0.8rem;
    font-weight: 600;
  }

  .btn-open-file:hover {
    background-color: var(--color-accent);
    color: #ffffff;
  }

  .icon-btn {
    width: 2rem;
    height: 2rem;
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .icon-btn:hover {
    background-color: var(--color-bg-alt);
    color: var(--color-text);
  }

  .icon-btn-accent {
    color: var(--color-accent);
  }
  .icon-btn-accent:hover {
    background-color: var(--color-accent-light);
    color: var(--color-accent);
  }

  .icon-btn-danger {
    color: #dc2626 !important;
  }
  .icon-btn-danger:hover {
    background-color: rgba(220, 38, 38, 0.12);
    color: #dc2626 !important;
  }

  .icon-btn-danger.confirming {
    width: auto;
    padding: 0.25rem 0.65rem;
    background-color: #dc2626;
    color: #ffffff !important;
    font-weight: 700;
    font-size: 0.75rem;
    animation: pulse-danger 1s infinite alternate;
  }

  @keyframes pulse-danger {
    from { opacity: 0.85; }
    to { opacity: 1; transform: scale(1.03); }
  }

  .confirm-text {
    white-space: nowrap;
  }

  .progress-bar-wrap {
    height: 6px;
    background-color: var(--color-bg-alt);
    border-radius: var(--radius-full);
    overflow: hidden;
  }

  .progress-bar-fill {
    height: 100%;
    border-radius: var(--radius-full);
    background: linear-gradient(135deg, #00b6bd, #0b1a4e) !important;
    transition: width 280ms cubic-bezier(0.4, 0, 0.2, 1);
    position: relative;
    overflow: hidden;
  }

  .progress-bar-fill.completed {
    background: #16a34a !important;
  }

  /* Light shimmer sweep across progress bar ONLY while downloading */
  @keyframes shimmer-sweep {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(100%);
    }
  }

  .progress-bar-fill.downloading::after {
    content: "";
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: linear-gradient(
      90deg,
      transparent 0%,
      rgba(255, 255, 255, 0.35) 50%,
      transparent 100%
    );
    animation: shimmer-sweep 2.2s infinite ease-in-out;
  }

  .card-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 0.785rem;
    color: var(--color-text-muted);
  }

  .metrics {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .dot-separator {
    opacity: 0.4;
  }

  .percent-badge {
    color: var(--color-accent);
    font-weight: 700;
    margin-inline-start: 0.35rem;
  }

  .speed-metric {
    font-weight: 600;
    color: var(--color-text);
  }

  .error-msg {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    color: var(--color-danger);
    font-size: 0.75rem;
    font-weight: 600;
    max-width: 50%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  @media (prefers-reduced-motion: reduce) {
    .task-card {
      animation: none !important;
    }
    .progress-bar-fill {
      transition: none !important;
    }
    .progress-bar-fill.downloading::after {
      display: none !important;
      animation: none !important;
    }
    .icon-btn-danger.confirming {
      animation: none !important;
    }
  }
</style>

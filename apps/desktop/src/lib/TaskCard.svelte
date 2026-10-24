<script>
  import { t, currentLang } from "./i18n.js";
  import { formatBytes, formatSpeed, formatEta, canPause, canResume, canCancel } from "./format.js";
  import StatusPill from "./StatusPill.svelte";
  import { createEventDispatcher } from "svelte";

  export let task;
  export let selected = false;

  const dispatch = createEventDispatcher();

  $: writtenFormatted = formatBytes(task.written, $currentLang);
  $: totalFormatted = task.total_size ? formatBytes(task.total_size, $currentLang) : $t("task.unknownSize");
  $: speedFormatted = formatSpeed(task.speed_bps, $currentLang);
  $: etaFormatted = formatEta(task.eta_secs, $currentLang);

  $: percent = task.total_size && task.total_size > 0
    ? Math.min(100, Math.max(0, Math.round((task.written / task.total_size) * 1000) / 10))
    : null;

  $: allowPause = canPause(task.state);
  $: allowResume = canResume(task.state);
  $: allowCancel = canCancel(task.state);
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
      <div class="file-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="20" height="20">
          <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
          <path d="M14 2v6h6M16 13H8M16 17H8M10 9H8" />
        </svg>
      </div>
      <div class="file-details">
        <span class="file-name" title={task.filename}>{task.filename || "—"}</span>
        <span class="file-url" title={task.url}>{task.url}</span>
      </div>
    </div>

    <div class="card-status-actions">
      <StatusPill state={task.state} />

      <div class="action-buttons">
        {#if allowPause}
          <button
            class="icon-btn"
            title={$t("task.pause")}
            aria-label="{$t('task.pause')}: {task.filename}"
            on:click|stopPropagation={() => dispatch("pause", task.id)}
          >
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16">
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
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16">
              <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM9.555 7.168A1 1 0 008 8v4a1 1 0 001.555.832l3-2a1 1 0 000-1.664l-3-2z" clip-rule="evenodd" />
            </svg>
          </button>
        {/if}

        {#if allowCancel}
          <button
            class="icon-btn icon-btn-danger"
            title={$t("task.cancel")}
            aria-label="{$t('task.cancel')}: {task.filename}"
            on:click|stopPropagation={() => dispatch("cancel", task.id)}
          >
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16">
              <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clip-rule="evenodd" />
            </svg>
          </button>
        {/if}

        <button
          class="icon-btn"
          title={$t("task.revealFolder")}
          aria-label="{$t('task.revealFolder')}: {task.filename}"
          on:click|stopPropagation={() => dispatch("reveal", task.id)}
        >
          <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16">
            <path fill-rule="evenodd" d="M2 6a2 2 0 012-2h4l2 2h6a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" clip-rule="evenodd" />
          </svg>
        </button>
      </div>
    </div>
  </div>

  <div class="progress-bar-wrap">
    <div
      class="progress-bar-fill"
      class:indeterminate={percent === null && task.state === 'Downloading'}
      class:completed={task.state === 'Completed'}
      style:width="{percent !== null ? percent : (task.state === 'Completed' ? 100 : 0)}%"
    ></div>
  </div>

  <div class="card-footer">
    <div class="metrics">
      <span class="size-metric">
        {writtenFormatted} / {totalFormatted}
        {#if percent !== null}
          <strong class="percent-badge">({percent}%)</strong>
        {/if}
      </span>

      {#if task.state === 'Downloading'}
        <span class="dot-separator">•</span>
        <span class="speed-metric">{speedFormatted}</span>
        <span class="dot-separator">•</span>
        <span class="eta-metric">{etaFormatted}</span>
      {/if}
    </div>

    {#if task.error}
      <div class="error-msg" title={task.error}>
        <svg viewBox="0 0 20 20" fill="currentColor" width="14" height="14">
          <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clip-rule="evenodd" />
        </svg>
        <span>{task.error}</span>
      </div>
    {/if}
  </div>
</div>

<style>
  .task-card {
    background-color: var(--color-bg-card);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: 1rem 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    box-shadow: var(--shadow-sm);
    transition: border var(--transition-normal), box-shadow var(--transition-normal);
    cursor: pointer;
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
    width: 2.25rem;
    height: 2.25rem;
    border-radius: var(--radius-sm);
    background-color: var(--color-bg-alt);
    color: var(--color-accent);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .file-details {
    display: flex;
    flex-direction: column;
    min-width: 0;
    gap: 0.15rem;
  }

  .file-name {
    font-size: 0.95rem;
    font-weight: 600;
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

  .icon-btn {
    width: 2rem;
    height: 2rem;
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
  }

  .icon-btn:hover {
    background-color: var(--color-bg-alt);
    color: var(--color-text);
  }

  .icon-btn-accent:hover {
    background-color: var(--color-accent-light);
    color: var(--color-accent);
  }

  .icon-btn-danger:hover {
    background-color: var(--color-danger-light);
    color: var(--color-danger);
  }

  .progress-bar-wrap {
    height: 0.375rem;
    background-color: var(--color-bg-alt);
    border-radius: var(--radius-full);
    overflow: hidden;
  }

  .progress-bar-fill {
    height: 100%;
    background-color: var(--color-accent);
    border-radius: var(--radius-full);
    transition: width 200ms ease;
  }

  .progress-bar-fill.completed {
    background-color: var(--color-success);
  }

  .progress-bar-fill.indeterminate {
    width: 40% !important;
    animation: indeterminate 1.5s infinite ease-in-out;
  }

  @keyframes indeterminate {
    0% { transform: translateX(-100%); }
    100% { transform: translateX(250%); }
  }

  .card-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 0.8rem;
    color: var(--color-text-muted);
  }

  .metrics {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .percent-badge {
    color: var(--color-text);
    margin-inline-start: 0.25rem;
  }

  .dot-separator {
    color: var(--color-text-subtle);
  }

  .error-msg {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    color: var(--color-danger);
    font-size: 0.75rem;
    max-width: 40%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>

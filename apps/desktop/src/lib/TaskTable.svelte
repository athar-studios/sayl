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

  export let tasks = [];
  export let selectedId = null;

  const dispatch = createEventDispatcher();

  function getPercent(task) {
    if (!task.total_size || task.total_size <= 0) return null;
    return Math.min(100, Math.max(0, Math.round((task.written / task.total_size) * 1000) / 10));
  }
</script>

<div class="table-container" role="region" aria-label={$t("view.table")}>
  <table class="tasks-table">
    <thead>
      <tr>
        <th class="col-name" scope="col">{$t("table.name")}</th>
        <th class="col-size" scope="col">{$t("table.size")}</th>
        <th class="col-progress" scope="col">{$t("table.progress")}</th>
        <th class="col-speed" scope="col">{$t("table.speed")}</th>
        <th class="col-eta" scope="col">{$t("table.eta")}</th>
        <th class="col-status" scope="col">{$t("table.status")}</th>
        <th class="col-actions" scope="col">{$t("table.actions")}</th>
      </tr>
    </thead>
    <tbody>
      {#each tasks as task (task.id)}
        {@const percent = getPercent(task)}
        {@const isSelected = selectedId === task.id}
        {@const allowPause = canPause(task.state)}
        {@const allowResume = canResume(task.state)}
        {@const allowCancel = canCancel(task.state)}
        {@const isCompleted = task.state === "Completed"}
        <tr
          class="task-row"
          class:selected={isSelected}
          on:click={() => dispatch("select", task.id)}
          tabindex="0"
          on:keydown={(e) => {
            if (e.key === "Enter") dispatch("select", task.id);
          }}
        >
          <td class="col-name cell-name">
            <span class="file-name" title={task.filename}><bdi>{task.filename || "—"}</bdi></span>
          </td>

          <td class="col-size cell-size">
            <span class="size-text">
              {#if task.total_size}
                <bdi>{formatBytes(task.total_size, $currentLang)}</bdi>
              {:else}
                <bdi>{$t("task.unknownSize")}</bdi>
              {/if}
            </span>
          </td>

          <td class="col-progress cell-progress">
            <div class="progress-wrapper">
              <div class="progress-track" role="progressbar" aria-valuenow={percent ?? 0} aria-valuemin="0" aria-valuemax="100">
                <div
                  class="progress-fill"
                  class:indeterminate={percent === null && (task.state === 'Downloading' || task.state === 'Probing')}
                  style="width: {percent !== null ? percent + '%' : '0%'};"
                />
              </div>
              <span class="percent-label">
                {#if percent !== null}
                  <bdi>{formatPercent(percent, $currentLang)}</bdi>
                {:else}
                  <bdi>—</bdi>
                {/if}
              </span>
            </div>
          </td>

          <td class="col-speed cell-speed">
            <span class="speed-text">
              {#if task.state === "Downloading" && task.speed_bps}
                <bdi>{formatSpeed(task.speed_bps, $currentLang)}</bdi>
              {:else}
                <bdi>—</bdi>
              {/if}
            </span>
          </td>

          <td class="col-eta cell-eta">
            <span class="eta-text">
              {#if task.state === "Downloading" && task.eta_secs}
                <bdi>{formatEta(task.eta_secs, $currentLang)}</bdi>
              {:else}
                <bdi>—</bdi>
              {/if}
            </span>
          </td>

          <td class="col-status cell-status">
            <StatusPill state={task.state} />
          </td>

          <td class="col-actions cell-actions">
            <div class="action-buttons">
              {#if allowPause}
                <button
                  class="action-btn"
                  title={$t("task.pause")}
                  aria-label="{$t('task.pause')}: {task.filename}"
                  on:click|stopPropagation={() => dispatch("pause", task.id)}
                >
                  <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15" aria-hidden="true">
                    <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zM7 8a1 1 0 012 0v4a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v4a1 1 0 102 0V8a1 1 0 00-1-1z" clip-rule="evenodd" />
                  </svg>
                </button>
              {/if}

              {#if allowResume}
                <button
                  class="action-btn action-btn-accent"
                  title={$t("task.resume")}
                  aria-label="{$t('task.resume')}: {task.filename}"
                  on:click|stopPropagation={() => dispatch("resume", task.id)}
                >
                  <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15" aria-hidden="true">
                    <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM9.555 7.168A1 1 0 008 8v4a1 1 0 001.555.832l3-2a1 1 0 000-1.664l-3-2z" clip-rule="evenodd" />
                  </svg>
                </button>
              {/if}

              {#if allowCancel}
                <button
                  class="action-btn action-btn-danger"
                  title={$t("task.cancel")}
                  aria-label="{$t('task.cancel')}: {task.filename}"
                  on:click|stopPropagation={() => dispatch("cancel", task.id)}
                >
                  <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15" aria-hidden="true">
                    <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clip-rule="evenodd" />
                  </svg>
                </button>
              {/if}

              <button
                class="action-btn"
                title={isCompleted ? $t("task.openFile") : $t("task.revealFolder")}
                aria-label="{isCompleted ? $t('task.openFile') : $t('task.revealFolder')}: {task.filename}"
                on:click|stopPropagation={() => dispatch("reveal", task.id)}
              >
                <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15" aria-hidden="true">
                  <path fill-rule="evenodd" d="M2 6a2 2 0 012-2h4l2 2h6a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" clip-rule="evenodd" />
                </svg>
              </button>
            </div>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .table-container {
    width: 100%;
    overflow-x: auto;
    background-color: var(--color-bg-card);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-sm);
  }

  .tasks-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.825rem;
    text-align: start;
  }

  thead {
    background-color: var(--color-bg-alt);
    border-bottom: 1px solid var(--color-border);
  }

  th {
    padding: 0.65rem 0.85rem;
    font-weight: 700;
    color: var(--color-text-muted);
    white-space: nowrap;
    text-align: start;
  }

  td {
    padding: 0.6rem 0.85rem;
    border-bottom: 1px solid var(--color-border);
    vertical-align: middle;
  }

  .task-row {
    transition: background-color var(--transition-normal);
    cursor: pointer;
  }

  .task-row:hover {
    background-color: var(--color-bg-card-hover);
  }

  .task-row.selected {
    background-color: var(--color-accent-light);
  }

  .col-name {
    min-width: 180px;
    max-width: 280px;
  }

  .file-name {
    display: block;
    font-weight: 600;
    color: var(--color-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .col-size {
    min-width: 90px;
    white-space: nowrap;
  }

  .col-progress {
    min-width: 160px;
  }

  .progress-wrapper {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .progress-track {
    flex: 1;
    height: 7px;
    background-color: var(--color-bg-alt);
    border-radius: var(--radius-full);
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--color-accent-gradient);
    border-radius: var(--radius-full);
    transition: width 200ms ease;
  }

  .progress-fill.indeterminate {
    width: 30% !important;
    animation: indeterminate-bar 1.5s infinite linear;
  }

  .percent-label {
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--color-text-muted);
    min-width: 2.75rem;
    text-align: end;
  }

  .col-speed,
  .col-eta {
    min-width: 95px;
    white-space: nowrap;
    color: var(--color-text-muted);
  }

  .col-status {
    min-width: 100px;
  }

  .col-actions {
    min-width: 120px;
  }

  .action-buttons {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  .action-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.75rem;
    height: 1.75rem;
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
    background: transparent;
    transition: background-color var(--transition-normal), color var(--transition-normal);
  }

  .action-btn:hover {
    background-color: var(--color-bg-alt);
    color: var(--color-text);
  }

  .action-btn-accent:hover {
    background-color: var(--color-accent-light);
    color: var(--color-accent);
  }

  .action-btn-danger:hover {
    background-color: var(--color-danger-light);
    color: var(--color-danger);
  }

  @keyframes indeterminate-bar {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(350%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .progress-fill {
      transition: none !important;
      animation: none !important;
    }
    .task-row,
    .action-btn {
      transition: none !important;
    }
  }
</style>

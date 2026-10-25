<script>
  import { t, currentLang } from "./i18n.js";
  import {
    formatBytes,
    formatSpeed,
    formatEta,
    formatPercent,
    formatDate,
    canPauseTask,
    canResumeTask,
    canCancel,
  } from "./format.js";
  import { friendlyError } from "./errors.js";
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
        <!-- Requirement 4: Exact column order and labels:
             الاسم (name), الحجم (size), الحالة (state), التقدّم (progress), السرعة (speed), المتبقي (eta), التاريخ (date) -->
        <th class="col-name" scope="col">{$t("table.name")}</th>
        <th class="col-size" scope="col">{$t("table.size")}</th>
        <th class="col-status" scope="col">{$t("table.status")}</th>
        <th class="col-progress" scope="col">{$t("table.progress")}</th>
        <th class="col-speed" scope="col">{$t("table.speed")}</th>
        <th class="col-eta" scope="col">{$t("table.eta")}</th>
        <th class="col-date" scope="col">{$t("table.date")}</th>
        <!-- Row actions accessible without a labeled column header -->
        <th class="col-actions" aria-hidden="true"></th>
      </tr>
    </thead>
    <tbody>
      {#each tasks as task (task.id)}
        {@const percent = getPercent(task)}
        {@const isSelected = selectedId === task.id}
        {@const allowPause = canPauseTask(task.state)}
        {@const allowResume = canResumeTask(task.state)}
        {@const isFailed = task.state === "Failed"}
        {@const isCompleted = task.state === "Completed"}
        {@const displayDate = isCompleted ? task.completed_at : task.created_at}
        <tr
          class="task-row"
          class:selected={isSelected}
          class:failed-row={isFailed}
          on:click={() => dispatch("select", task.id)}
          tabindex="0"
          on:keydown={(e) => {
            if (e.key === "Enter") dispatch("select", task.id);
          }}
        >
          <!-- 1. الاسم -->
          <td class="col-name cell-name">
            <span class="file-name" title={task.filename}><bdi>{task.filename || "—"}</bdi></span>
            {#if isFailed}
              {@const err = friendlyError(task, $currentLang)}
              <div class="friendly-error-block">
                <div class="err-text-row">
                  <span class="err-title">{err.title}</span>
                  <span class="err-hint">{err.hint}</span>
                </div>
                {#if task.error}
                  <details class="err-details">
                    <summary class="err-summary" on:click|stopPropagation>{$t("errors.details")}</summary>
                    <pre class="err-raw" dir="auto"><code>{task.error}</code></pre>
                  </details>
                {/if}
              </div>
            {/if}
          </td>

          <!-- 2. الحجم -->
          <td class="col-size cell-size">
            <span class="size-text">
              {#if task.total_size}
                <bdi>{formatBytes(task.total_size, $currentLang)}</bdi>
              {:else}
                <bdi>{$t("task.unknownSize")}</bdi>
              {/if}
            </span>
          </td>

          <!-- 3. الحالة -->
          <td class="col-status cell-status">
            <StatusPill state={task.state} />
          </td>

          <!-- 4. التقدّم -->
          <td class="col-progress cell-progress">
            <div class="progress-wrapper">
              <div
                class="progress-track"
                role="progressbar"
                aria-valuenow={percent ?? 0}
                aria-valuemin="0"
                aria-valuemax="100"
              >
                <div
                  class="progress-fill"
                  class:indeterminate={percent === null && (task.state === 'Downloading' || task.state === 'Probing')}
                  class:completed={isCompleted}
                  style="width: {percent !== null ? percent + '%' : (isCompleted ? '100%' : '0%')};"
                ></div>
              </div>
              <span class="percent-label">
                {#if percent !== null}
                  <bdi>{formatPercent(percent, $currentLang)}</bdi>
                {:else if isCompleted}
                  <bdi>{formatPercent(100, $currentLang)}</bdi>
                {:else}
                  <bdi>—</bdi>
                {/if}
              </span>
            </div>
          </td>

          <!-- 5. السرعة -->
          <td class="col-speed cell-speed">
            <span class="speed-text">
              {#if task.state === "Downloading" && task.speed_bps}
                <bdi>{formatSpeed(task.speed_bps, $currentLang)}</bdi>
              {:else}
                <bdi>—</bdi>
              {/if}
            </span>
          </td>

          <!-- 6. المتبقي -->
          <td class="col-eta cell-eta">
            <span class="eta-text">
              {#if task.state === "Downloading" && task.eta_secs}
                <bdi>{formatEta(task.eta_secs, $currentLang)}</bdi>
              {:else}
                <bdi>—</bdi>
              {/if}
            </span>
          </td>

          <!-- 7. التاريخ -->
          <td class="col-date cell-date">
            <span class="date-text">
              <bdi>{formatDate(displayDate, $currentLang)}</bdi>
            </span>
          </td>

          <!-- Row actions (pause/resume/retry/delete/reveal/open) -->
          <td class="col-actions cell-actions">
            <div class="action-buttons">
              {#if allowPause}
                <button
                  class="action-btn"
                  title={$t("toolbar.pause")}
                  aria-label="{$t('toolbar.pause')}: {task.filename}"
                  on:click|stopPropagation={() => dispatch("pause", task.id)}
                >
                  <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15" aria-hidden="true">
                    <path
                      fill-rule="evenodd"
                      d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zM7 8a1 1 0 012 0v4a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v4a1 1 0 102 0V8a1 1 0 00-1-1z"
                      clip-rule="evenodd"
                    />
                  </svg>
                </button>
              {/if}

              {#if allowResume && !isFailed}
                <button
                  class="action-btn action-btn-accent"
                  title={$t("toolbar.resume")}
                  aria-label="{$t('toolbar.resume')}: {task.filename}"
                  on:click|stopPropagation={() => dispatch("resume", task.id)}
                >
                  <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15" aria-hidden="true">
                    <path
                      fill-rule="evenodd"
                      d="M10 18a8 8 0 100-16 8 8 0 000 16zM9.555 7.168A1 1 0 008 8v4a1 1 0 001.555.832l3-2a1 1 0 000-1.664l-3-2z"
                      clip-rule="evenodd"
                    />
                  </svg>
                </button>
              {/if}

              {#if isFailed}
                <button
                  class="action-btn action-btn-retry"
                  title={$t("table.retry")}
                  aria-label="{$t('table.retry')}: {task.filename}"
                  on:click|stopPropagation={() => dispatch("retry", task.id)}
                >
                  <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15" aria-hidden="true">
                    <path
                      fill-rule="evenodd"
                      d="M4 2a1 1 0 011 1v2.101a7.002 7.002 0 0111.601 2.566 1 1 0 11-1.885.666A5.002 5.002 0 005.999 7H9a1 1 0 010 2H4a1 1 0 01-1-1V3a1 1 0 011-1zm.008 9.057a1 1 0 011.276.61A5.002 5.002 0 0014.001 13H11a1 1 0 110-2h5a1 1 0 011 1v5a1 1 0 11-2 0v-2.101a7.002 7.002 0 01-11.601-2.566 1 1 0 01.61-1.276z"
                      clip-rule="evenodd"
                    />
                  </svg>
                </button>
              {/if}

              <button
                class="action-btn action-btn-danger"
                title={$t("table.delete")}
                aria-label="{$t('table.delete')}: {task.filename}"
                on:click|stopPropagation={() => dispatch("cancel", task.id)}
              >
                <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15" aria-hidden="true">
                  <path
                    fill-rule="evenodd"
                    d="M9 2a1 1 0 00-.894.553L7.382 4H4a1 1 0 000 2v10a2 2 0 002 2h8a2 2 0 002-2V6a1 1 0 100-2h-3.382l-.724-1.447A1 1 0 0011 2H9zM7 8a1 1 0 012 0v6a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v6a1 1 0 102 0V8a1 1 0 00-1-1z"
                    clip-rule="evenodd"
                  />
                </svg>
              </button>

              <button
                class="action-btn"
                title={isCompleted ? $t("task.openFile") : $t("task.revealFolder")}
                aria-label="{isCompleted ? $t('task.openFile') : $t('task.revealFolder')}: {task.filename}"
                on:click|stopPropagation={() => {
                  if (isCompleted) {
                    dispatch("openFile", task.id);
                  } else {
                    dispatch("reveal", task.id);
                  }
                }}
              >
                <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15" aria-hidden="true">
                  <path
                    fill-rule="evenodd"
                    d="M2 6a2 2 0 012-2h4l2 2h6a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z"
                    clip-rule="evenodd"
                  />
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
    padding: 0.65rem 0.85rem;
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
    background-color: var(--color-accent-light) !important;
  }

  .task-row:focus-visible {
    outline: 2px solid var(--color-accent);
    outline-offset: -2px;
  }

  .col-name {
    min-width: 190px;
    max-width: 300px;
  }

  .file-name {
    display: block;
    font-weight: 600;
    color: var(--color-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Friendly error inside table */
  .friendly-error-block {
    margin-top: 0.35rem;
    padding: 0.35rem 0.5rem;
    background-color: var(--color-danger-light);
    border-radius: var(--radius-sm);
    font-size: 0.75rem;
  }

  .err-text-row {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  .err-title {
    font-weight: 700;
    color: var(--color-danger);
  }

  .err-hint {
    color: var(--color-text-muted);
    line-height: 1.35;
  }

  .err-details {
    margin-top: 0.35rem;
    cursor: pointer;
  }

  .err-summary {
    font-size: 0.725rem;
    font-weight: 700;
    color: var(--color-text-muted);
    user-select: none;
  }

  .err-summary:hover {
    color: var(--color-text);
  }

  .err-raw {
    margin-top: 0.3rem;
    padding: 0.35rem 0.5rem;
    background-color: rgba(0, 0, 0, 0.08);
    border-radius: 4px;
    font-family: monospace;
    font-size: 0.7rem;
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 5rem;
    overflow-y: auto;
    color: var(--color-text);
  }

  :global([data-theme="dark"]) .err-raw {
    background-color: rgba(0, 0, 0, 0.35);
  }

  .col-size {
    min-width: 85px;
    white-space: nowrap;
  }

  .col-status {
    min-width: 105px;
  }

  .col-progress {
    min-width: 150px;
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
    background: var(--progress-gradient, var(--color-accent-gradient));
    border-radius: var(--radius-full);
    transition: width 200ms ease;
  }

  .progress-fill.completed {
    background: var(--state-completed, #16a34a) !important;
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
    min-width: 90px;
    white-space: nowrap;
    color: var(--color-text-muted);
  }

  .col-date {
    min-width: 125px;
    white-space: nowrap;
    color: var(--color-text-muted);
    font-size: 0.775rem;
  }

  .col-actions {
    min-width: 110px;
  }

  .action-buttons {
    display: flex;
    align-items: center;
    justify-content: flex-end;
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

  .action-btn-retry:hover {
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

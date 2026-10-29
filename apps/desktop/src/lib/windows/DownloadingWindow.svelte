<script>
  /**
   * IDM-style "Downloading" window.
   *
   * Layout: title, tab strip («حالة التنزيل» / «حدود السرعة»), big progress bar,
   * segmented connection strip, status rows, per-connection table and the action
   * row («إخفاء التفاصيل» / «إيقاف مؤقت» / «إلغاء»).
   *
   * Progress is merged monotonically (`mergeTaskMonotonic`) so it can never move
   * backwards while the same task keeps its state. The header X and «إخفاء»
   * dispatch `close` (hide only); «إلغاء» dispatches `cancel`.
   */
  import { createEventDispatcher } from "svelte";
  import { t, currentLang } from "../i18n.js";
  import { formatBytes, formatPercent, isTaskPending } from "../format.js";
  import { friendlyError } from "../errors.js";
  import { mergeTaskMonotonic } from "../monotonic.js";
  import {
    buildStatusRows,
    buildConnectionSegments,
    buildConnectionTableRows,
    buildSegmentBars,
    progressPercent,
    getWindowActionFlags,
    buildSpeedLimitState,
  } from "../downloadWindowUi.js";
  import StatusPill from "../StatusPill.svelte";
  import iconSvg from "../../assets/icon.svg";

  export let task;
  export let autoShutdown = false;
  export let autoClose = false;
  export let isNativeWindow = false;
  export let pendingStart = null;
  /** From settings; used only for the read-only «حدود السرعة» tab. */
  export let maxConnections = null;
  /** Optional KB/s cap already configured for this task. */
  export let speedLimitKbps = null;

  const dispatch = createEventDispatcher();

  /** @type {'status'|'speedLimit'} */
  let activeTab = "status";
  let showDetails = true;

  // Monotonically merge incoming task updates so progress never regresses.
  let mergedTask = task;
  $: mergedTask = mergeTaskMonotonic(mergedTask, task);

  $: isRtl = $currentLang === "ar";
  $: isPending = isTaskPending(pendingStart, mergedTask?.id);
  $: isFailed = mergedTask?.state === "Failed";
  $: errInfo = isFailed ? friendlyError(mergedTask, $currentLang) : null;

  $: percentNum = progressPercent(mergedTask?.written, mergedTask?.total_size);
  $: percentText = percentNum === null ? "—" : formatPercent(percentNum, $currentLang);

  $: statusRows = buildStatusRows(mergedTask, $currentLang);
  $: segments = buildConnectionSegments(mergedTask);
  // One cell per connection region; no byte-band binning so each cell is a
  // connection, exactly like IDM's segmented strip.
  $: segBars = buildSegmentBars(segments, {
    maxCells: Math.max(1, Math.min(32, segments.length || 1)),
  });
  $: connRows = buildConnectionTableRows(segments, $currentLang);
  $: actionFlags = getWindowActionFlags(mergedTask, { isPending });
  $: speedState = buildSpeedLimitState({ speed_limit_kbps: speedLimitKbps }, $currentLang);
  $: writtenText = formatBytes(mergedTask?.written ?? 0, $currentLang);
  $: totalText =
    mergedTask?.total_size && mergedTask.total_size > 0
      ? formatBytes(mergedTask.total_size, $currentLang)
      : $t("task.unknownSize");
  $: connectionsLabel = connRows.length || mergedTask?.parts || mergedTask?.connections || 0;

  function statusLabel(status) {
    if (status === "done") return $t("downloadWindow.connDone");
    if (status === "downloading") return $t("downloadWindow.connDownloading");
    return $t("downloadWindow.connPending");
  }

  function rowValue(row) {
    if (row.key !== "state") return row.value;
    return $t(`state.${row.value}`);
  }

  function hideWindow() {
    dispatch("close", mergedTask.id);
  }
</script>

<div
  class="idm-window"
  class:rtl={isRtl}
  class:floating={!isNativeWindow}
  dir={isRtl ? "rtl" : "ltr"}
  role="dialog"
  aria-label="{percentText} {mergedTask?.filename || $t('downloadWindow.title')}"
>
  <!-- Title row -->
  <div class="title-row">
    <div class="brand">
      <img src={iconSvg} alt="SAYL" class="brand-logo" aria-hidden="true" />
      <span class="file-name" title={mergedTask?.filename}>
        <bdi dir="auto">{mergedTask?.filename || "—"}</bdi>
      </span>
    </div>
    <div class="title-end">
      <StatusPill state={mergedTask?.state} isConnecting={isPending} />
      <button class="icon-btn" title={$t("downloadWindow.hide")} aria-label={$t("downloadWindow.hide")} on:click={hideWindow}>
        <svg viewBox="0 0 20 20" fill="currentColor" width="14" height="14" aria-hidden="true">
          <path
            fill-rule="evenodd"
            d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z"
            clip-rule="evenodd"
          />
        </svg>
      </button>
    </div>
  </div>

  <!-- Tabs -->
  <div class="tab-strip" role="tablist">
    <button
      class="tab"
      class:active={activeTab === "status"}
      role="tab"
      aria-selected={activeTab === "status"}
      on:click={() => (activeTab = "status")}
    >
      {$t("downloadWindow.tabs.status")}
    </button>
    <button
      class="tab"
      class:active={activeTab === "speedLimit"}
      role="tab"
      aria-selected={activeTab === "speedLimit"}
      on:click={() => (activeTab = "speedLimit")}
    >
      {$t("downloadWindow.tabs.speedLimit")}
    </button>
  </div>

  <div class="tab-body">
    {#if activeTab === "status"}
      <!-- Big progress bar -->
      <div class="progress-block">
        <div class="progress-track" role="progressbar" aria-valuenow={percentNum ?? 0} aria-valuemin="0" aria-valuemax="100">
          <div class="progress-fill" style="width: {percentNum ?? 0}%"></div>
          <span class="progress-caption">
            <bdi dir="ltr">{writtenText} / {totalText}</bdi>
            <bdi dir="ltr" class="progress-pct">{percentText}</bdi>
          </span>
        </div>
      </div>

      <!-- Status rows -->
      <div class="status-grid">
        {#each statusRows as row (row.key)}
          <div class="status-row">
            <span class="status-label">{$t(row.labelKey)}:</span>
            <span class="status-value" dir={row.valueDir}>
              <bdi dir={row.valueDir}>{rowValue(row)}</bdi>
            </span>
          </div>
        {/each}
      </div>

      {#if isFailed}
        <div class="error-banner" role="alert">
          <span class="error-text" title={mergedTask?.error || errInfo?.hint || ""}>
            <bdi dir="auto">{mergedTask?.error || errInfo?.hint || $t("state.Failed")}</bdi>
          </span>
        </div>
      {/if}

      <!-- Segmented connection strip -->
      {#if segBars.length > 0}
        <div class="segment-strip" aria-label={$t("downloadWindow.connections")}>
          {#each segBars as cell (cell.key)}
            <div class="segment-cell" title={`#${cell.label} · ${Math.round(cell.fraction * 100)}%`}>
              <div class="segment-fill" style="width: {Math.round(cell.fraction * 100)}%"></div>
            </div>
          {/each}
        </div>
        <div class="segment-legend">
          <span>{$t("downloadWindow.connections")}: <bdi dir="ltr">{connectionsLabel}</bdi></span>
        </div>
      {/if}

      <!-- Per-connection table -->
      {#if showDetails}
        <div class="conn-table-wrap">
          <table class="conn-table">
            <thead>
              <tr>
                <th class="col-num" scope="col">{$t("downloadWindow.connIndex")}</th>
                <th class="col-downloaded" scope="col">{$t("downloadWindow.connDownloaded")}</th>
                <th class="col-info" scope="col">{$t("downloadWindow.connInfo")}</th>
                <th class="col-status" scope="col">{$t("downloadWindow.connStatus")}</th>
              </tr>
            </thead>
            <tbody>
              {#if connRows.length === 0}
                <tr>
                  <td class="empty-row" colspan="4">{$t("downloadWindow.connPending")}</td>
                </tr>
              {:else}
                {#each connRows as conn (conn.key)}
                  <tr>
                    <td class="col-num"><bdi dir="ltr">{conn.index}</bdi></td>
                    <td class="col-downloaded"><bdi dir="ltr">{conn.downloaded}</bdi></td>
                    <td class="col-info"><bdi dir="ltr">{conn.info}</bdi></td>
                    <td class="col-status">
                      <span class="conn-status" class:done={conn.status === "done"} class:active={conn.status === "downloading"}>
                        {statusLabel(conn.status)}
                      </span>
                    </td>
                  </tr>
                {/each}
              {/if}
            </tbody>
          </table>
        </div>
      {/if}
    {:else}
      <!-- «حدود السرعة» tab: read-only summary of the configured limits -->
      <div class="status-grid">
        <div class="status-row">
          <span class="status-label">{$t("downloadWindow.speedLimit")}:</span>
          <span class="status-value" dir="ltr"><bdi dir="ltr">{speedState.label}</bdi></span>
        </div>
        <div class="status-row">
          <span class="status-label">{$t("downloadWindow.maxConnections")}:</span>
          <span class="status-value" dir="ltr">
            <bdi dir="ltr">{connectionsLabel > 0 ? connectionsLabel : (maxConnections ?? "—")}</bdi>
          </span>
        </div>
      </div>
      <p class="tab-note">{$t("downloadWindow.speedLimitNote")}</p>
    {/if}
  </div>

  <!-- Actions -->
  <div class="action-row">
    {#if activeTab === "status"}
      <button class="btn btn-secondary" on:click={() => (showDetails = !showDetails)}>
        {showDetails ? $t("downloadWindow.hideDetails") : $t("downloadWindow.showDetails")}
      </button>
    {/if}

    {#if actionFlags.pause}
      <button class="btn btn-secondary" on:click={() => dispatch("pause", mergedTask.id)}>
        {$t("downloadWindow.pause")}
      </button>
    {/if}

    {#if actionFlags.resume}
      <button
        class="btn btn-primary"
        disabled={isPending}
        on:click={() => dispatch("resume", mergedTask.id)}
      >
        {isPending ? $t("task.connecting") : $t("downloadWindow.resume")}
      </button>
    {/if}

    {#if actionFlags.retry}
      <button
        class="btn btn-primary"
        disabled={isPending}
        on:click={() => dispatch("retry", mergedTask.id)}
      >
        {isPending ? $t("task.connecting") : $t("downloadWindow.retry")}
      </button>
    {/if}

    <button class="btn btn-secondary" on:click={hideWindow}>{$t("downloadWindow.hide")}</button>

    {#if actionFlags.cancel}
      <button class="btn btn-danger" on:click={() => dispatch("cancel", mergedTask.id)}>
        {$t("downloadWindow.cancel")}
      </button>
    {/if}
  </div>

  <div class="option-row">
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

<style>
  /* Navy #0b1a4e and turquoise #00b6bd appear as accents only. */
  .idm-window {
    --navy: #0b1a4e;
    --turquoise: #00b6bd;
    box-sizing: border-box;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    background: var(--color-bg-card, #ffffff);
    color: var(--text, #1f2937);
    border: 1px solid var(--color-border, #d8dee9);
    border-radius: 0;
    font-family: var(--font-family, "Segoe UI", Tahoma, Arial, sans-serif);
    font-size: 0.8125rem;
    user-select: none;
    overflow: hidden;
    pointer-events: auto;
  }

  .idm-window.rtl {
    text-align: right;
  }

  /* Floating (in-app) size; the native window stretches to fill its webview. */
  .idm-window.floating {
    width: 560px;
    min-width: 420px;
    height: 400px;
    min-height: 320px;
    border-radius: 8px;
    box-shadow: var(--shadow-md, 0 6px 18px rgba(11, 26, 78, 0.18));
  }

  /* Title row */
  .title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    flex-shrink: 0;
    min-width: 0;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    overflow: hidden;
  }

  .brand-logo {
    width: 18px;
    height: 18px;
    object-fit: contain;
    flex-shrink: 0;
  }

  .file-name {
    font-weight: 600;
    color: var(--navy);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .idm-window.rtl .file-name {
    color: var(--navy);
  }

  .title-end {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .icon-btn {
    width: 22px;
    height: 22px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    color: var(--text-muted, #6b7280);
    border: 1px solid transparent;
    border-radius: 4px;
    cursor: pointer;
  }

  .icon-btn:hover {
    background: var(--color-bg-card-hover, #eef1f6);
    color: var(--navy);
    border-color: var(--color-border, #d8dee9);
  }

  /* Tabs */
  .tab-strip {
    display: flex;
    gap: 2px;
    border-bottom: 1px solid var(--color-border, #d8dee9);
    flex-shrink: 0;
  }

  .tab {
    padding: 5px 12px;
    background: transparent;
    border: 1px solid transparent;
    border-bottom: none;
    border-radius: 6px 6px 0 0;
    color: var(--text-muted, #6b7280);
    font-family: inherit;
    font-size: 0.8125rem;
    font-weight: 600;
    cursor: pointer;
  }

  .tab:hover {
    color: var(--navy);
  }

  .tab.active {
    color: var(--navy);
    background: var(--color-bg-card, #ffffff);
    border-color: var(--color-border, #d8dee9);
    box-shadow: inset 0 2px 0 0 var(--turquoise);
  }

  .tab-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  /* Progress */
  .progress-block {
    flex-shrink: 0;
  }

  .progress-track {
    position: relative;
    height: 22px;
    width: 100%;
    background: var(--color-bg-subtle, #eef1f6);
    border: 1px solid var(--color-border, #d8dee9);
    border-radius: 4px;
    overflow: hidden;
  }

  .idm-window.rtl .progress-track {
    direction: rtl;
  }

  .progress-fill {
    height: 100%;
    background: var(--turquoise);
    transition: width 200ms linear;
  }

  .progress-caption {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 8px;
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--navy);
    font-variant-numeric: tabular-nums;
    pointer-events: none;
    direction: ltr;
  }

  .progress-pct {
    color: #ffffff;
    text-shadow: 0 0 2px rgba(11, 26, 78, 0.65);
  }

  /* Status rows */
  .status-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 4px 14px;
    flex-shrink: 0;
  }

  .status-row {
    display: flex;
    align-items: baseline;
    gap: 5px;
    min-width: 0;
    line-height: 1.45;
  }

  .status-label {
    color: var(--turquoise);
    font-weight: 600;
    flex-shrink: 0;
    white-space: nowrap;
  }

  .status-value {
    color: var(--navy);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .idm-window.rtl .status-value[dir="ltr"] {
    direction: ltr;
    unicode-bidi: isolate;
  }

  /* Error */
  .error-banner {
    padding: 5px 8px;
    border: 1px solid var(--color-danger, #b42318);
    border-radius: 4px;
    background: var(--color-danger-light, #fdecea);
    color: var(--color-danger, #b42318);
    font-size: 0.75rem;
    flex-shrink: 0;
  }

  .error-text {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Segmented connection strip */
  .segment-strip {
    display: flex;
    gap: 2px;
    height: 14px;
    flex-shrink: 0;
    border: 1px solid var(--color-border, #d8dee9);
    border-radius: 3px;
    padding: 1px;
    background: var(--color-bg-subtle, #eef1f6);
    overflow: hidden;
  }

  .segment-cell {
    flex: 1 1 0;
    min-width: 2px;
    background: var(--color-border, #d8dee9);
    border-radius: 1px;
    overflow: hidden;
  }

  .segment-fill {
    height: 100%;
    background: var(--turquoise);
    transition: width 200ms linear;
  }

  .segment-legend {
    display: flex;
    justify-content: flex-end;
    color: var(--text-muted, #6b7280);
    font-size: 0.75rem;
    flex-shrink: 0;
  }

  .idm-window.rtl .segment-legend {
    justify-content: flex-start;
  }

  /* Per-connection table */
  .conn-table-wrap {
    flex: 1;
    min-height: 80px;
    overflow: auto;
    border: 1px solid var(--color-border, #d8dee9);
    border-radius: 4px;
  }

  .conn-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.75rem;
    font-variant-numeric: tabular-nums;
  }

  .conn-table thead th {
    position: sticky;
    top: 0;
    background: var(--color-bg-subtle, #eef1f6);
    color: var(--navy);
    font-weight: 600;
    text-align: start;
    padding: 3px 6px;
    border-bottom: 1px solid var(--color-border, #d8dee9);
    white-space: nowrap;
  }

  .conn-table tbody td {
    padding: 2px 6px;
    border-bottom: 1px solid var(--color-border-subtle, #edf0f4);
    color: var(--text, #1f2937);
    text-align: start;
    white-space: nowrap;
  }

  .conn-table tbody tr:last-child td {
    border-bottom: none;
  }

  .col-num {
    width: 34px;
    text-align: center;
  }

  .col-downloaded {
    width: 96px;
  }

  .col-status {
    width: 96px;
  }

  .empty-row {
    text-align: center !important;
    color: var(--text-muted, #6b7280);
    padding: 10px 6px !important;
  }

  .conn-status {
    color: var(--text-muted, #6b7280);
  }

  .conn-status.active {
    color: var(--turquoise);
    font-weight: 600;
  }

  .conn-status.done {
    color: var(--navy);
    font-weight: 600;
  }

  .tab-note {
    margin: 0;
    color: var(--text-muted, #6b7280);
    font-size: 0.75rem;
  }

  /* Actions */
  .action-row {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .btn {
    min-width: 92px;
    height: 28px;
    padding: 0 12px;
    border-radius: 4px;
    font-family: inherit;
    font-size: 0.8125rem;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid var(--color-border, #d8dee9);
    transition: background-color 120ms ease;
  }

  .btn-secondary {
    background: var(--color-bg-card, #ffffff);
    color: var(--navy);
  }

  .btn-secondary:hover {
    background: var(--color-bg-card-hover, #eef1f6);
  }

  .btn-primary {
    background: var(--navy);
    color: #ffffff;
    border-color: var(--navy);
  }

  .btn-primary:hover {
    background: #16296b;
  }

  .btn-danger {
    background: var(--color-bg-card, #ffffff);
    color: var(--color-danger, #b42318);
    border-color: var(--color-danger, #b42318);
    margin-inline-start: auto;
  }

  .btn-danger:hover {
    background: var(--color-danger-light, #fdecea);
  }

  .btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .option-row {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
    flex-shrink: 0;
  }

  .option-label {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 0.75rem;
    color: var(--text-muted, #6b7280);
    cursor: pointer;
  }

  .option-checkbox {
    width: 14px;
    height: 14px;
    cursor: pointer;
    accent-color: var(--turquoise);
  }

  @media (prefers-reduced-motion: reduce) {
    .progress-fill,
    .segment-fill {
      transition: none !important;
    }
  }
</style>

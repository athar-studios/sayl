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
    createConnectionTableTracker,
    buildSegmentBars,
    taskPercent,
    getWindowActionFlags,
    buildSpeedLimitState,
    parseSpeedLimitInput,
    formatSpeedLimit,
    kbpsToDisplayUnit,
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

  // Never report 100% while the task is still probing («فحص الرابط...»):
  // the size is unknown at that point, so the percent stays empty.
  $: percentNum = taskPercent(mergedTask);
  $: percentText = percentNum === null ? "—" : formatPercent(percentNum, $currentLang);

  $: statusRows = buildStatusRows(mergedTask, $currentLang);
  $: segments = buildConnectionSegments(mergedTask);
  // One cell per connection region; no byte-band binning so each cell is a
  // connection, exactly like IDM's segmented strip.
  $: segBars = buildSegmentBars(segments, {
    maxCells: Math.max(1, Math.min(32, segments.length || 1)),
  });
  // Live per-connection table: one row per connection, fed by every task
  // update. The tracker keeps each connection's bytes monotonic for the same
  // attempt and restarts them on a retry; it tolerates a payload without any
  // per-range data by synthesising bands from `parts` / `connections`.
  const connectionTable = createConnectionTableTracker();
  $: connRows = connectionTable.rows(mergedTask, $currentLang);
  $: actionFlags = getWindowActionFlags(mergedTask, { isPending });
  $: speedState = buildSpeedLimitState({ speed_limit_kbps: speedLimitKbps }, $currentLang);

  // Speed limit tab state
  let speedLimitEnabled = Boolean(speedLimitKbps && speedLimitKbps > 0);
  let { value: speedLimitValue, unit: speedLimitUnit } = kbpsToDisplayUnit(speedLimitKbps);
  let speedLimitError = "";
  let speedLimitSaving = false;

  function parseAndValidateInput() {
    const res = parseSpeedLimitInput(speedLimitValue, speedLimitUnit);
    if (!res.ok) {
      speedLimitError = $t(`downloadWindow.speedLimitError.${res.error}`);
      return null;
    }
    speedLimitError = "";
    return res.kbps; // null means unlimited
  }

  async function applySpeedLimit() {
    const kbps = parseAndValidateInput();
    if (kbps === null && speedLimitError) return;
    if (kbps === undefined) return;

    speedLimitSaving = true;
    try {
      if (typeof window !== "undefined" && (window.__TAURI_INTERNALS__ || window.__TAURI__)) {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("set_task_speed_limit", {
          id: mergedTask.id,
          kbps,
        });
      }
      // Update local state immediately for responsive UI
      speedLimitKbps = kbps;
      speedLimitEnabled = kbps !== null;
      // Persist the "remember" preference for this task
      if (typeof window !== "undefined" && kbps !== null) {
        try {
          localStorage.setItem(`sayl:speedLimit:${mergedTask.id}`, String(kbps));
        } catch {}
      }
    } catch (e) {
      console.error("Failed to set speed limit:", e);
      speedLimitError = $t("downloadWindow.speedLimitError.applyFailed");
    } finally {
      speedLimitSaving = false;
    }
  }

  function onSpeedLimitEnableChange() {
    if (!speedLimitEnabled) {
      applySpeedLimit(); // Will pass null (unlimited)
    }
  }

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

        <!-- Per-task speed limit on status tab -->
        {#if speedLimitKbps !== null && speedLimitKbps !== undefined}
          <div class="status-row">
            <span class="status-label">{$t("downloadWindow.speedLimit")}:</span>
            <span class="status-value" dir="auto"><bdi>{formatSpeedLimit(speedLimitKbps, $currentLang)}</bdi></span>
          </div>
        {/if}
      </div>

      {#if isFailed}
        <div class="error-banner" role="alert">
          <span class="error-text" title={mergedTask?.error || errInfo?.hint || ""}>
            <bdi dir="auto">{mergedTask?.error || errInfo?.hint || $t("state.Failed")}</bdi>
          </span>
        </div>
      {/if}
    {:else}
      <!-- «حدود السرعة» tab: interactive per-task speed limit like IDM -->
      <div class="speed-limit-form">
        <div class="option-row speed-limit-toggle">
          <label class="option-label">
            <input
              type="checkbox"
              class="option-checkbox"
              checked={speedLimitEnabled}
              on:change={onSpeedLimitEnableChange}
            />
            <span>{$t("downloadWindow.speedLimitEnable")}</span>
          </label>
        </div>

        <div class="speed-limit-inputs" class:disabled={!speedLimitEnabled}>
          <div class="input-group">
            <label for="speedLimitValue" class="input-label">
              {$t("downloadWindow.speedLimitKbps")}
            </label>
            <div class="input-with-unit">
              <input
                id="speedLimitValue"
                type="number"
                class="speed-input"
                bind:value={speedLimitValue}
                min="0"
                step="1"
                disabled={!speedLimitEnabled}
                placeholder={$t("downloadWindow.unlimited")}
              />
              <select
                class="unit-select"
                bind:value={speedLimitUnit}
                disabled={!speedLimitEnabled}
              >
                <option value="kbps">{$t("units.kbps")}</option>
                <option value="mbps">{$t("units.mbps")}</option>
              </select>
            </div>
          </div>

          <div class="input-group">
            <label class="input-label remember-label">
              <input
                type="checkbox"
                class="option-checkbox"
                checked={speedLimitKbps !== null}
                disabled={!speedLimitEnabled}
              />
              <span>{$t("downloadWindow.rememberLimit")}</span>
            </label>
          </div>

          {#if speedLimitError}
            <div class="error-message">{speedLimitError}</div>
          {/if}
        </div>

        <div class="speed-limit-actions">
          <button
            class="btn btn-primary"
            on:click={applySpeedLimit}
            disabled={!speedLimitEnabled || speedLimitSaving || speedLimitError}
          >
            {speedLimitSaving ? $t("common.saving") : $t("downloadWindow.speedLimitSave")}
          </button>
          <span class="current-limit-hint">
            {$t("downloadWindow.currentLimit")}: {formatSpeedLimit(speedLimitKbps, $currentLang)}
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
  /* Navy #0b1a4e and turquoise #00b6bd appear as accents only (progress fill, primary button, focus).
     All text/surface/border colors come from the app's theme CSS variables so both light/dark work. */
  .idm-window {
    box-sizing: border-box;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    background: var(--color-bg-card);
    color: var(--color-text);
    border: 1px solid var(--color-border);
    border-radius: 0;
    font-family: var(--font-family);
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
    box-shadow: var(--shadow-md);
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
    color: var(--color-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
    color: var(--color-text-muted);
    border: 1px solid transparent;
    border-radius: 4px;
    cursor: pointer;
  }

  .icon-btn:hover {
    background: var(--color-bg-card-hover);
    color: var(--color-text);
    border-color: var(--color-border);
  }

  /* Tabs */
  .tab-strip {
    display: flex;
    gap: 2px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .tab {
    padding: 5px 12px;
    background: transparent;
    border: 1px solid transparent;
    border-bottom: none;
    border-radius: 6px 6px 0 0;
    color: var(--color-text-muted);
    font-family: inherit;
    font-size: 0.8125rem;
    font-weight: 600;
    cursor: pointer;
  }

  .tab:hover {
    color: var(--color-text);
  }

  .tab.active {
    color: var(--color-text);
    background: var(--color-bg-card);
    border-color: var(--color-border);
    box-shadow: inset 0 2px 0 0 var(--color-accent);
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
    background: var(--color-bg-subtle);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    overflow: hidden;
  }

  .idm-window.rtl .progress-track {
    direction: rtl;
  }

  .progress-fill {
    height: 100%;
    background: var(--color-accent);
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
    color: var(--color-text);
    font-variant-numeric: tabular-nums;
    pointer-events: none;
    direction: ltr;
  }

  /* The caption sits over both the empty track and the turquoise fill, so each
     value gets a card-coloured pill: --color-text on --color-bg-card stays
     >= 7.5:1 in light and >= 9.9:1 in dark no matter what is underneath. */
  .progress-caption bdi {
    background: var(--color-bg-card);
    color: var(--color-text);
    padding: 0 5px;
    border-radius: 3px;
    line-height: 1.35;
  }

  .progress-pct {
    font-weight: 700;
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
    color: var(--color-text-muted);
    font-weight: 600;
    flex-shrink: 0;
    white-space: nowrap;
  }

  .status-value {
    color: var(--color-text);
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
    border: 1px solid var(--color-danger);
    border-radius: 4px;
    background: var(--color-danger-light);
    color: var(--color-text);
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
    border: 1px solid var(--color-border);
    border-radius: 3px;
    padding: 1px;
    background: var(--color-bg-subtle);
    overflow: hidden;
  }

  .segment-cell {
    flex: 1 1 0;
    min-width: 2px;
    background: var(--color-border);
    border-radius: 1px;
    overflow: hidden;
  }

  .segment-fill {
    height: 100%;
    background: var(--color-accent);
    transition: width 200ms linear;
  }

  .segment-legend {
    display: flex;
    justify-content: flex-end;
    color: var(--color-text-muted);
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
    border: 1px solid var(--color-border);
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
    background: var(--color-bg-subtle);
    color: var(--color-text-muted);
    font-weight: 600;
    text-align: start;
    padding: 3px 6px;
    border-bottom: 1px solid var(--color-border);
    white-space: nowrap;
  }

  .conn-table tbody td {
    padding: 2px 6px;
    border-bottom: 1px solid var(--color-border-subtle);
    color: var(--color-text);
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
    color: var(--color-text-muted) !important;
    padding: 10px 6px !important;
  }

  /* Status text stays readable (AA); the accent only marks the active row. */
  .conn-status {
    color: var(--color-text-muted);
  }

  .conn-status.active {
    color: var(--color-text);
    font-weight: 600;
  }

  .conn-status.active::before {
    content: "•";
    color: var(--color-accent);
    margin-inline-end: 4px;
  }

  .conn-status.done {
    color: var(--color-text);
    font-weight: 600;
  }

  .tab-note {
    margin: 0;
    color: var(--color-text-muted);
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
    border: 1px solid var(--color-border);
    transition: background-color 120ms ease;
  }

  /* Secondary buttons read as normal text on the card surface in both themes;
     the border uses --color-text-subtle so it clears 3:1 against the card in
     light (4.8:1) and dark (5.8:1) instead of vanishing in the dark palette. */
  .btn-secondary {
    background: var(--color-bg-card);
    color: var(--color-text);
    border-color: var(--color-text-subtle);
  }

  .btn-secondary:hover:not(:disabled) {
    background: var(--color-bg-card-hover);
  }

  /* Navy stays the accent for the primary action (white on #0b1a4e = 16.4:1). */
  .btn-primary {
    background: #0b1a4e;
    color: #ffffff;
    border-color: #0b1a4e;
  }

  .btn-primary:hover:not(:disabled) {
    background: #16296b;
    border-color: #16296b;
  }

  .btn-danger {
    background: var(--color-bg-card);
    color: var(--color-danger);
    border-color: var(--color-danger);
    margin-inline-start: auto;
  }

  .btn-danger:hover:not(:disabled) {
    background: var(--color-danger-light);
  }

  /* Disabled: visibly inert yet still legible (muted text on the subtle surface). */
  .btn:disabled,
  .btn:disabled:hover {
    background: var(--color-bg-subtle);
    color: var(--color-text-muted);
    border-color: var(--color-text-subtle);
    border-style: dashed;
    opacity: 1;
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
    color: var(--color-text-muted);
    cursor: pointer;
  }

  .option-checkbox {
    width: 14px;
    height: 14px;
    cursor: pointer;
    accent-color: var(--color-accent);
  }

  @media (prefers-reduced-motion: reduce) {
    .progress-fill,
    .segment-fill {
      transition: none !important;
    }
  }
  /* Speed-limit tab controls: theme variables only, readable in light and dark. */
  .speed-limit-form { display: flex; flex-direction: column; gap: 10px; color: var(--color-text); }
  .speed-limit-inputs, .input-with-unit, .speed-limit-actions { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .speed-input, .unit-select {
    font: inherit;
    padding: 4px 8px;
    color: var(--color-text);
    background: var(--color-bg-card);
    border: 1px solid var(--color-border);
    border-radius: 4px;
  }
  .speed-input { width: 120px; }
  .speed-input:focus-visible, .unit-select:focus-visible { outline: 2px solid var(--color-accent, #00b6bd); outline-offset: 1px; }
  .speed-input:disabled, .unit-select:disabled { opacity: 0.6; }
  .input-label { color: var(--color-text-muted); }
  .current-limit-hint { color: var(--color-text-muted); font-size: 0.85rem; }
  .error-message { color: var(--color-danger); font-size: 0.85rem; }

  /* Dark theme keeps every text element on the theme's light text colour.
     --color-danger is a semantic accent, so on a dark card the red moves to the
     border/marker and the label itself uses --color-text (13.34:1 on #1f2937)
     instead of #f87171 (5.31:1). Light theme is untouched. Both the explicit
     [data-theme="dark"] attribute (set by the app and by the standalone windows)
     and a dark OS preference without an attribute are covered. */
  @media (prefers-color-scheme: dark) {
    :global(:root:not([data-theme="light"])) .error-message,
    :global(:root:not([data-theme="light"])) .btn-danger {
      color: var(--color-text);
    }
  }
  :global([data-theme="dark"]) .error-message,
  :global([data-theme="dark"]) .btn-danger {
    color: var(--color-text);
  }
</style>

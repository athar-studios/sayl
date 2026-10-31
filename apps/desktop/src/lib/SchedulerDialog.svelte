<script>
  import { t } from "./i18n.js";
  import { safeInvoke } from "./tauri.js";
  import {
    loadSchedule,
    saveSchedule,
    serializeTime,
    parseTime,
    DEFAULT_SCHEDULE,
  } from "./scheduler.js";
  import { createEventDispatcher } from "svelte";

  export let open = false;

  const dispatch = createEventDispatcher();

  let schedule = { ...DEFAULT_SCHEDULE };
  let startTimeStr = "02:00";
  let stopTimeStr = "06:00";
  let saving = false;
  let saveSuccess = false;

  const DAYS = [
    { id: 1, key: "scheduler.mon" },
    { id: 2, key: "scheduler.tue" },
    { id: 3, key: "scheduler.wed" },
    { id: 4, key: "scheduler.thu" },
    { id: 5, key: "scheduler.fri" },
    { id: 6, key: "scheduler.sat" },
    { id: 7, key: "scheduler.sun" },
  ];

  $: if (open) {
    schedule = loadSchedule();
    startTimeStr = serializeTime(schedule.start_hour, schedule.start_minute);
    stopTimeStr = serializeTime(schedule.stop_hour, schedule.stop_minute);
    saveSuccess = false;
  }

  function handleDayToggle(dayId) {
    if (schedule.days.includes(dayId)) {
      schedule.days = schedule.days.filter((d) => d !== dayId);
    } else {
      schedule.days = [...schedule.days, dayId].sort((a, b) => a - b);
    }
  }

  function handleClose() {
    dispatch("close");
  }

  async function handleSave() {
    saving = true;
    try {
      const parsedStart = parseTime(startTimeStr);
      const parsedStop = parseTime(stopTimeStr);

      const payload = {
        ...schedule,
        start_hour: parsedStart.hour,
        start_minute: parsedStart.minute,
        stop_hour: parsedStop.hour,
        stop_minute: parsedStop.minute,
      };

      saveSchedule(payload);
      schedule = payload;

      try {
        await safeInvoke("set_schedule", { schedule: payload });
      } catch {
        // Fallback safely if backend command does not exist
      }

      saveSuccess = true;
      setTimeout(() => {
        saveSuccess = false;
        handleClose();
      }, 350);
    } finally {
      saving = false;
    }
  }

  function handleKeydown(e) {
    if (!open) return;
    if (e.key === "Escape") {
      e.preventDefault();
      handleClose();
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if open}
  <div class="modal-backdrop" on:click={handleClose} on:keydown={handleKeydown} role="presentation">
    <div
      class="dialog-card"
      on:click|stopPropagation
      on:keydown|stopPropagation
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-labelledby="scheduler-title"
    >
      <!-- Header -->
      <div class="dialog-header">
        <div class="header-icon-wrap" aria-hidden="true">
          <svg viewBox="0 0 20 20" fill="currentColor" width="20" height="20">
            <path
              fill-rule="evenodd"
              d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-12a1 1 0 10-2 0v4a1 1 0 00.293.707l2.828 2.829a1 1 0 101.415-1.415L11 9.586V6z"
              clip-rule="evenodd"
            />
          </svg>
        </div>
        <h3 id="scheduler-title" class="dialog-title">{$t("scheduler.title")}</h3>
        <button
          type="button"
          class="close-btn"
          on:click={handleClose}
          aria-label={$t("scheduler.cancel")}
        >
          <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18" aria-hidden="true">
            <path
              fill-rule="evenodd"
              d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z"
              clip-rule="evenodd"
            />
          </svg>
        </button>
      </div>

      <!-- Body -->
      <div class="dialog-body">
        <!-- Times row -->
        <div class="form-row times-row">
          <div class="form-col">
            <label for="sched-start-time" class="field-label">{$t("scheduler.startTime")}</label>
            <input
              id="sched-start-time"
              type="time"
              class="form-control"
              bind:value={startTimeStr}
            />
          </div>

          <div class="form-col">
            <label for="sched-stop-time" class="field-label">{$t("scheduler.stopTime")}</label>
            <input
              id="sched-stop-time"
              type="time"
              class="form-control"
              bind:value={stopTimeStr}
            />
          </div>
        </div>

        <!-- Days row -->
        <div class="form-section">
          <span class="field-label">{$t("scheduler.days")}</span>
          <div class="days-grid">
            {#each DAYS as day}
              <label class="day-check-label">
                <input
                  type="checkbox"
                  class="day-checkbox"
                  checked={schedule.days.includes(day.id)}
                  on:change={() => handleDayToggle(day.id)}
                />
                <span class="day-name">{$t(day.key)}</span>
              </label>
            {/each}
          </div>
        </div>

        <!-- Queue size row -->
        <div class="form-row align-center">
          <label for="sched-queue-size" class="field-label">{$t("scheduler.queueSize")}</label>
          <input
            id="sched-queue-size"
            type="number"
            min="1"
            max="10"
            class="form-control queue-input"
            bind:value={schedule.queue_size}
          />
        </div>

        <!-- Shutdown when done toggle -->
        <div class="form-row align-center shutdown-row">
          <label class="toggle-wrap">
            <input
              type="checkbox"
              class="styled-checkbox"
              bind:checked={schedule.shutdown_when_done}
            />
            <span class="field-label no-margin">{$t("scheduler.shutdownWhenDone")}</span>
          </label>
        </div>

        {#if saveSuccess}
          <div class="success-banner" role="status">
            <span>{$t("scheduler.savedNotice")}</span>
          </div>
        {/if}
      </div>

      <!-- Footer -->
      <div class="dialog-footer">
        <button
          type="button"
          class="btn btn-secondary"
          on:click={handleClose}
        >
          {$t("scheduler.cancel")}
        </button>
        <button
          type="button"
          class="btn btn-primary"
          disabled={saving}
          on:click={handleSave}
        >
          {#if saving}
            <span>...</span>
          {:else}
            <span>{$t("scheduler.save")}</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background-color: rgba(7, 15, 46, 0.65);
    backdrop-filter: blur(3px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 150;
    padding: 1rem;
  }

  .dialog-card {
    background: #ffffff;
    border: 1px solid #d2d6dc;
    border-radius: 6px;
    box-shadow: 0 10px 25px rgba(0, 0, 0, 0.2);
    width: 100%;
    max-width: 480px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    font-family: var(--font-system, "Segoe UI", Arial, sans-serif);
    color: #1a1a1a;
  }

  :global([data-theme="dark"]) .dialog-card {
    background: #0d1a44;
    border-color: #1e3a8a;
    color: #f1f5f9;
  }

  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.85rem 1.15rem;
    background: #f8fafc;
    border-bottom: 1px solid #e2e8f0;
    gap: 0.5rem;
  }

  :global([data-theme="dark"]) .dialog-header {
    background: #091338;
    border-bottom-color: #162b6e;
  }

  .header-icon-wrap {
    color: #00b6bd;
    display: flex;
    align-items: center;
  }

  .dialog-title {
    font-size: 1rem;
    font-weight: 700;
    margin: 0;
    flex: 1;
  }

  .close-btn {
    background: transparent;
    border: none;
    cursor: pointer;
    color: #64748b;
    padding: 4px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close-btn:hover {
    background: rgba(0, 0, 0, 0.06);
    color: #0f172a;
  }

  :global([data-theme="dark"]) .close-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #f8fafc;
  }

  .dialog-body {
    padding: 1.15rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .form-row {
    display: flex;
    gap: 1rem;
  }

  .form-row.align-center {
    align-items: center;
    justify-content: space-between;
  }

  .times-row {
    display: flex;
    gap: 1rem;
  }

  .form-col {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .field-label {
    font-size: 0.825rem;
    font-weight: 600;
    color: #475569;
  }

  :global([data-theme="dark"]) .field-label {
    color: #cbd5e1;
  }

  .field-label.no-margin {
    margin: 0;
  }

  .form-control {
    padding: 0.45rem 0.65rem;
    border: 1px solid #cbd5e1;
    border-radius: 4px;
    font-size: 0.875rem;
    background: #ffffff;
    color: #1e293b;
  }

  :global([data-theme="dark"]) .form-control {
    background: #070e28;
    border-color: #1e3a8a;
    color: #f1f5f9;
  }

  .queue-input {
    width: 80px;
    text-align: center;
  }

  .form-section {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .days-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.5rem;
  }

  .day-check-label {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.8rem;
    cursor: pointer;
    user-select: none;
  }

  .day-checkbox,
  .styled-checkbox {
    accent-color: #00b6bd;
    cursor: pointer;
    width: 15px;
    height: 15px;
  }

  .toggle-wrap {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
    user-select: none;
  }

  .success-banner {
    background: rgba(16, 185, 129, 0.15);
    color: #10b981;
    padding: 0.5rem 0.75rem;
    border-radius: 4px;
    font-size: 0.825rem;
    font-weight: 600;
    text-align: center;
  }

  .dialog-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.5rem;
    padding: 0.85rem 1.15rem;
    background: #f8fafc;
    border-top: 1px solid #e2e8f0;
  }

  :global([data-theme="dark"]) .dialog-footer {
    background: #091338;
    border-top-color: #162b6e;
  }

  .btn {
    padding: 0.45rem 1rem;
    border-radius: 4px;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
    border: none;
    transition: all 120ms ease;
  }

  .btn-primary {
    background: #00b6bd;
    color: #ffffff;
  }

  .btn-primary:hover:not(:disabled) {
    background: #029ca2;
  }

  .btn-secondary {
    background: #f1f5f9;
    border: 1px solid #cbd5e1;
    color: #334155;
  }

  :global([data-theme="dark"]) .btn-secondary {
    background: #102052;
    border-color: #1e3a8a;
    color: #cbd5e1;
  }

  .btn-secondary:hover:not(:disabled) {
    background: #e2e8f0;
  }

  :global([data-theme="dark"]) .btn-secondary:hover:not(:disabled) {
    background: #172b68;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>

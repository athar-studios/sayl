<script>
  import { onDestroy, createEventDispatcher } from "svelte";
  import { t } from "./i18n.js";
  import { safeInvoke } from "./tauri.js";
  import { DEFAULT_EXTENSION_PATH } from "./browsers.js";
  import {
    EXTENSION_GUIDE_TOTAL_STEPS,
    buildStepCounterLabel,
    mapPingResultToStatus,
    resolveExtensionGuideDisplay,
  } from "./extensionGuide.js";

  export let open = false;
  export let browser = { id: "chrome", name: "" };
  export let extensionPath = "";

  const dispatch = createEventDispatcher();

  let activeStep = 1;
  let copied = false;
  let copyTimeout = null;
  let pollInterval = null;
  let dialogElement = null;
  let previousActiveElement = null;
  let resolvedPath = extensionPath || DEFAULT_EXTENSION_PATH;
  /** @type {{ ok?: boolean, message?: string } | null} */
  let pingResult = null;
  let pingLoaded = false;
  /** @type {number | null} */
  let extensionSeenAgeMs = null;
  let seenAgeLoaded = false;

  $: if (extensionPath) {
    resolvedPath = extensionPath;
  }

  $: browserLabel = browser?.name?.trim() || $t("extension.defaultBrowser");
  $: stepCounterLabel = buildStepCounterLabel(
    activeStep,
    EXTENSION_GUIDE_TOTAL_STEPS,
    $t("tour.stepCounter")
  );
  $: statusDisplay = resolveExtensionGuideDisplay(
    mapPingResultToStatus(pingLoaded ? pingResult : null),
    seenAgeLoaded ? extensionSeenAgeMs : null
  );

  $: if (open) {
    initDialog();
  } else {
    cleanupDialog();
  }

  async function initDialog() {
    activeStep = 1;
    copied = false;
    pingResult = null;
    pingLoaded = false;
    extensionSeenAgeMs = null;
    seenAgeLoaded = false;
    previousActiveElement = document.activeElement;

    if (!resolvedPath || resolvedPath === DEFAULT_EXTENSION_PATH) {
      try {
        const prep = await safeInvoke("prepare_extension");
        if (typeof prep === "string" && prep.trim()) {
          resolvedPath = prep.trim();
        }
      } catch {
        // keep fallback path
      }
    }

    startPolling();

    setTimeout(() => {
      if (dialogElement) {
        const firstBtn = dialogElement.querySelector("button");
        if (firstBtn) firstBtn.focus();
      }
    }, 50);
  }

  function cleanupDialog() {
    stopPolling();
    if (copyTimeout) clearTimeout(copyTimeout);
    if (previousActiveElement && typeof previousActiveElement.focus === "function") {
      try {
        previousActiveElement.focus();
      } catch {}
    }
  }

  async function refreshGuideStatus() {
    try {
      const res = await safeInvoke("test_native_host");
      pingResult =
        res && typeof res === "object"
          ? res
          : { ok: false, message: $t("extension.testFailed").replace("{msg}", "") };
    } catch (err) {
      pingResult = {
        ok: false,
        message: typeof err === "string" ? err : err?.message || String(err),
      };
    } finally {
      pingLoaded = true;
    }

    try {
      const age = await safeInvoke("extension_seen_age_ms");
      extensionSeenAgeMs = typeof age === "number" && Number.isFinite(age) ? age : null;
    } catch {
      extensionSeenAgeMs = null;
    } finally {
      seenAgeLoaded = true;
    }
  }

  function startPolling() {
    stopPolling();
    refreshGuideStatus();
    pollInterval = setInterval(refreshGuideStatus, 5000);
    if (typeof window !== "undefined") {
      window.addEventListener("focus", refreshGuideStatus);
    }
    if (typeof document !== "undefined") {
      document.addEventListener("visibilitychange", handleVisibility);
    }
  }

  function stopPolling() {
    if (pollInterval) {
      clearInterval(pollInterval);
      pollInterval = null;
    }
    if (typeof window !== "undefined") {
      window.removeEventListener("focus", refreshGuideStatus);
    }
    if (typeof document !== "undefined") {
      document.removeEventListener("visibilitychange", handleVisibility);
    }
  }

  function handleVisibility() {
    if (document.visibilityState === "visible") {
      refreshGuideStatus();
    }
  }

  async function handleCopyPath() {
    if (!resolvedPath) return;
    try {
      if (navigator?.clipboard?.writeText) {
        await navigator.clipboard.writeText(resolvedPath);
      } else {
        const textarea = document.createElement("textarea");
        textarea.value = resolvedPath;
        textarea.style.position = "fixed";
        textarea.style.opacity = "0";
        document.body.appendChild(textarea);
        textarea.select();
        document.execCommand("copy");
        document.body.removeChild(textarea);
      }
      copied = true;
      if (copyTimeout) clearTimeout(copyTimeout);
      copyTimeout = setTimeout(() => {
        copied = false;
      }, 2500);
    } catch {
      copied = false;
    }
  }

  async function handleOpenFolder() {
    try {
      await safeInvoke("open_extension_folder");
    } catch {
      // safeInvoke handles error toast
    }
  }

  async function handleOpenBrowserExtensions() {
    try {
      await safeInvoke("open_browser_extensions", { browserId: browser?.id || "chrome" });
    } catch {
      // safeInvoke handles error toast
    }
  }

  function handleClose() {
    dispatch("close");
  }

  function handleKeydown(e) {
    if (!open) return;
    if (e.key === "Escape") {
      e.stopPropagation();
      handleClose();
      return;
    }

    if (e.key === "Tab" && dialogElement) {
      const focusables = dialogElement.querySelectorAll(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
      );
      if (focusables.length === 0) return;
      const first = focusables[0];
      const last = focusables[focusables.length - 1];

      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  }

  onDestroy(() => {
    cleanupDialog();
  });
</script>

<svelte:window on:keydown={handleKeydown} />

{#if open}
  <div class="dialog-backdrop" on:click={handleClose} on:keydown={handleKeydown} role="presentation">
    <div
      bind:this={dialogElement}
      class="dialog-card"
      on:click|stopPropagation
      on:keydown|stopPropagation
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-labelledby="guide-title"
    >
      <div class="dialog-header">
        <h3 id="guide-title" class="dialog-title">
          {$t("extension.dialogTitle").replace("{browser}", browserLabel)}
        </h3>
        <button
          type="button"
          class="close-btn"
          on:click={handleClose}
          aria-label={$t("settings.close")}
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

      <div class="dialog-body">
        <div class="step-meta">
          <span class="step-counter">{stepCounterLabel}</span>
          <h4 class="step-title">
            {activeStep === 1 ? $t("extension.step1Title") : $t("extension.step2Title")}
          </h4>
        </div>

        {#if activeStep === 1}
          <p class="step-instruction">{$t("extension.step1Desc")}</p>
        {:else}
          <p class="step-instruction">{$t("extension.step2Desc")}</p>

          <div class="path-display-box">
            <label for="ext-path-input" class="path-label">{$t("extension.folderPath")}</label>
            <div class="path-input-group">
              <input
                id="ext-path-input"
                type="text"
                readonly
                dir="ltr"
                class="path-input"
                value={resolvedPath}
                on:click={(e) => e.target.select()}
              />
              <button
                type="button"
                class="btn-copy"
                class:copied
                on:click={handleCopyPath}
                title={$t("extension.copyPath")}
              >
                {#if copied}
                  <span>{$t("extension.pathCopied")}</span>
                {:else}
                  <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15" aria-hidden="true">
                    <path d="M8 3a1 1 0 011-1h2a1 1 0 110 2H9a1 1 0 01-1-1z" />
                    <path
                      d="M6 3a2 2 0 00-2 2v11a2 2 0 002 2h8a2 2 0 002-2V5a2 2 0 00-2-2 3 3 0 01-3 3H9a3 3 0 01-3-3z"
                    />
                  </svg>
                  <span>{$t("extension.copyPath")}</span>
                {/if}
              </button>
            </div>
          </div>
        {/if}

        <div class="guide-actions-bar">
          <button type="button" class="btn btn-secondary" on:click={handleOpenBrowserExtensions}>
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
              <path
                fill-rule="evenodd"
                d="M4.25 2A2.25 2.25 0 002 4.25v11.5A2.25 2.25 0 004.25 18h11.5A2.25 2.25 0 0018 15.75V4.25A2.25 2.25 0 0015.75 2H4.25zm4.03 6.28a.75.75 0 00-1.06-1.06L4.97 9.47a.75.75 0 000 1.06l2.25 2.25a.75.75 0 001.06-1.06L6.81 10.25h6.44a.75.75 0 000-1.5H6.81l1.47-1.47z"
                clip-rule="evenodd"
              />
            </svg>
            <span>{$t("extension.openBrowserPage")}</span>
          </button>

          <button type="button" class="btn btn-secondary" on:click={handleOpenFolder}>
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
              <path
                fill-rule="evenodd"
                d="M2 4.75C2 3.784 2.784 3 3.75 3h4.836c.464 0 .909.184 1.237.513l1.414 1.414a1.75 1.75 0 001.237.513H16.25c.966 0 1.75.784 1.75 1.75v7.06c0 .966-.784 1.75-1.75 1.75H3.75A1.75 1.75 0 012 14.31V4.75z"
                clip-rule="evenodd"
              />
            </svg>
            <span>{$t("extension.openFolder")}</span>
          </button>
        </div>

        <div class="status-return-hint">
          {#if statusDisplay.variant === "success"}
            <span class="status-indicator-badge success">
              <span class="pulse-dot"></span>
              {$t(statusDisplay.statusKey)} ✓
            </span>
          {:else if statusDisplay.variant === "error"}
            <span class="status-indicator-badge error">
              {statusDisplay.message}
            </span>
          {:else if statusDisplay.variant === "checking" || statusDisplay.variant === "ready"}
            <span class="status-indicator-badge checking">
              <span class="spin-dot"></span>
              {$t(statusDisplay.statusKey)}
            </span>
          {:else}
            <span class="status-indicator-badge checking">
              <span class="spin-dot"></span>
              {$t(statusDisplay.statusKey)}
            </span>
          {/if}
        </div>
      </div>

      <div class="dialog-footer">
        {#if activeStep === 1}
          <button type="button" class="btn btn-primary" on:click={() => (activeStep = 2)}>
            <span>{$t("tour.next")}</span>
            <span aria-hidden="true">→</span>
          </button>
        {:else}
          <button type="button" class="btn btn-secondary" on:click={() => (activeStep = 1)}>
            <span aria-hidden="true">←</span>
            <span>{$t("tour.prev")}</span>
          </button>
          <button type="button" class="btn btn-primary" on:click={handleClose}>
            <span>{$t("extension.guideClose")}</span>
          </button>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .dialog-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(11, 26, 78, 0.72);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    padding: 1rem;
  }

  .dialog-card {
    background: var(--color-bg-card, #0f1d46);
    color: var(--color-text, #e2e8f0);
    border: 1px solid var(--color-border, #23386e);
    border-radius: var(--radius-lg, 12px);
    box-shadow: var(--shadow-lg, 0 20px 40px rgba(0, 0, 0, 0.55));
    width: 100%;
    max-width: 520px;
    display: flex;
    flex-direction: column;
    max-height: min(90vh, 640px);
  }

  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 1rem 1.25rem;
    border-bottom: 1px solid var(--color-border, #1c2e5c);
  }

  .dialog-title {
    font-size: 1.05rem;
    font-weight: 700;
    margin: 0;
    flex: 1;
    color: var(--color-text, #ffffff);
  }

  .close-btn {
    background: transparent;
    border: none;
    color: var(--color-text-muted, #8b9fc7);
    cursor: pointer;
    padding: 0.35rem;
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close-btn:hover {
    background: var(--color-bg-hover, #192a56);
    color: var(--color-text, #ffffff);
  }

  .dialog-body {
    padding: 1.15rem 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
    overflow-y: auto;
  }

  .step-meta {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .step-counter {
    font-size: 0.75rem;
    font-weight: 700;
    color: #00b6bd;
    letter-spacing: 0.02em;
  }

  .step-title {
    margin: 0;
    font-size: 1rem;
    font-weight: 700;
    color: var(--color-text, #f8fafc);
  }

  .step-instruction {
    font-size: 0.95rem;
    line-height: 1.55;
    color: var(--color-text-muted, #d1dff8);
    margin: 0;
  }

  .path-display-box {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .path-label {
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--color-text-muted, #8ea5d0);
  }

  .path-input-group {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .path-input {
    flex: 1;
    min-width: 0;
    background: #08112c;
    border: 1px solid #23396c;
    border-radius: 6px;
    color: #00b6bd;
    font-family: monospace;
    font-size: 0.82rem;
    padding: 0.5rem 0.75rem;
    user-select: all;
  }

  .btn-copy {
    background: #1b2f60;
    border: 1px solid #2e4a8a;
    border-radius: 6px;
    color: #e2e8f0;
    font-size: 0.82rem;
    font-weight: 600;
    padding: 0 0.85rem;
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    cursor: pointer;
    white-space: nowrap;
    transition: background 150ms ease;
  }

  .btn-copy:hover {
    background: #233e7e;
  }

  .btn-copy.copied {
    background: #059669;
    border-color: #10b981;
    color: #ffffff;
  }

  .guide-actions-bar {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
    padding-top: 0.5rem;
    border-top: 1px solid #1a2b56;
  }

  .status-return-hint {
    background: rgba(11, 26, 78, 0.45);
    border: 1px dashed #203362;
    border-radius: 6px;
    padding: 0.65rem 0.9rem;
    font-size: 0.82rem;
    color: #9cb2db;
  }

  .status-indicator-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
  }

  .status-indicator-badge.success {
    color: #34d399;
    font-weight: 700;
  }

  .status-indicator-badge.checking {
    color: #8da4d2;
  }

  .status-indicator-badge.error {
    color: #fca5a5;
  }

  .pulse-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #34d399;
    box-shadow: 0 0 8px #34d399;
  }

  .spin-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 2px solid #00b6bd;
    border-top-color: transparent;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .dialog-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.75rem;
    padding: 0.85rem 1.25rem;
    background: rgba(11, 26, 78, 0.35);
    border-top: 1px solid var(--color-border, #1c2e5c);
  }

  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    padding: 0.5rem 1rem;
    border-radius: 6px;
    font-size: 0.88rem;
    font-weight: 600;
    cursor: pointer;
    border: none;
    transition: background 150ms ease;
  }

  .btn-primary {
    background: #00b6bd;
    color: #0b1a4e;
  }

  .btn-primary:hover {
    background: #0299a1;
    color: #ffffff;
  }

  .btn-secondary {
    background: #172750;
    color: #cbd5e1;
    border: 1px solid #283c70;
  }

  .btn-secondary:hover {
    background: #21356a;
    color: #ffffff;
  }
</style>

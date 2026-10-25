<script>
  import { onMount, onDestroy, createEventDispatcher } from "svelte";
  import { t } from "./i18n.js";
  import { safeInvoke } from "./tauri.js";
  import { DEFAULT_EXTENSION_PATH } from "./browsers.js";

  export let open = false;
  export let browser = { id: "chrome", name: "Google Chrome", connected: false };
  export let extensionPath = "";

  const dispatch = createEventDispatcher();

  let activeStep = 1;
  let copied = false;
  let copyTimeout = null;
  let pollInterval = null;
  let dialogElement = null;
  let previousActiveElement = null;
  let resolvedPath = extensionPath || DEFAULT_EXTENSION_PATH;

  $: if (extensionPath) {
    resolvedPath = extensionPath;
  }

  $: if (open) {
    initDialog();
  } else {
    cleanupDialog();
  }

  async function initDialog() {
    activeStep = 1;
    copied = false;
    previousActiveElement = document.activeElement;

    // Resolve path if not provided
    if (!resolvedPath || resolvedPath === DEFAULT_EXTENSION_PATH) {
      try {
        const prep = await safeInvoke("prepare_extension");
        if (typeof prep === "string" && prep.trim()) {
          resolvedPath = prep.trim();
        }
      } catch {
        // Fallback remains
      }
    }

    startPolling();

    // Trap focus to dialog on next tick
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

  function startPolling() {
    stopPolling();
    // Poll every 5 seconds
    pollInterval = setInterval(checkStatus, 5000);
    if (typeof window !== "undefined") {
      window.addEventListener("focus", checkStatus);
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
      window.removeEventListener("focus", checkStatus);
    }
    if (typeof document !== "undefined") {
      document.removeEventListener("visibilitychange", handleVisibility);
    }
  }

  function handleVisibility() {
    if (document.visibilityState === "visible") {
      checkStatus();
    }
  }

  function checkStatus() {
    dispatch("pollStatus");
  }

  async function handleCopyPath() {
    if (!resolvedPath) return;
    try {
      if (navigator?.clipboard?.writeText) {
        await navigator.clipboard.writeText(resolvedPath);
      } else {
        // Fallback
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
  <div class="guide-backdrop" on:click={handleClose} on:keydown={handleKeydown} role="presentation">
    <div
      bind:this={dialogElement}
      class="guide-dialog"
      on:click|stopPropagation
      on:keydown|stopPropagation
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-labelledby="guide-title"
    >
      <!-- Dialog Header -->
      <div class="guide-header">
        <div class="header-titles">
          <span class="guide-badge">2 {$t("tour.stepCounter").replace("{current}", activeStep).replace("{total}", "2")}</span>
          <h3 id="guide-title" class="guide-title">
            {$t("extension.dialogTitle").replace("{browser}", browser?.name || "Chromium")}
          </h3>
        </div>
        <button class="guide-close-btn" on:click={handleClose} aria-label={$t("settings.close")}>
          <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18" aria-hidden="true">
            <path
              fill-rule="evenodd"
              d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z"
              clip-rule="evenodd"
            />
          </svg>
        </button>
      </div>

      <!-- Step Navigation Tabs -->
      <div class="guide-steps-nav" role="tablist">
        <button
          class="step-tab"
          class:active={activeStep === 1}
          role="tab"
          aria-selected={activeStep === 1}
          on:click={() => (activeStep = 1)}
        >
          <span class="step-num">1</span>
          <span class="step-label">{$t("extension.step1Title")}</span>
        </button>
        <button
          class="step-tab"
          class:active={activeStep === 2}
          role="tab"
          aria-selected={activeStep === 2}
          on:click={() => (activeStep = 2)}
        >
          <span class="step-num">2</span>
          <span class="step-label">{$t("extension.step2Title")}</span>
        </button>
      </div>

      <!-- Step Content Area -->
      <div class="guide-body">
        {#if activeStep === 1}
          <div class="step-content">
            <p class="step-instruction">
              {$t("extension.step1Desc")}
            </p>

            <!-- Inline Illustrative SVG Panel: Developer Mode Toggle -->
            <div class="illustrative-panel" aria-hidden="true">
              <svg viewBox="0 0 460 140" class="panel-svg" fill="none" xmlns="http://www.w3.org/2000/svg">
                <!-- Browser Header Background -->
                <rect width="460" height="140" rx="8" fill="#0c193c" stroke="#1d2e5a" stroke-width="1.5" />
                <rect x="0" y="0" width="460" height="34" rx="8" fill="#14244d" />
                <!-- Window dots -->
                <circle cx="20" cy="17" r="4.5" fill="#f87171" opacity="0.8" />
                <circle cx="34" cy="17" r="4.5" fill="#fbbf24" opacity="0.8" />
                <circle cx="48" cy="17" r="4.5" fill="#34d399" opacity="0.8" />
                <!-- Search bar fake -->
                <rect x="75" y="9" width="160" height="16" rx="4" fill="#0c193c" opacity="0.7" />
                <rect x="85" y="14" width="70" height="6" rx="2" fill="#586f9c" />

                <!-- Extensions Title / Breadcrumb -->
                <text x="24" y="66" fill="#f1f5f9" font-size="14" font-weight="600" font-family="system-ui, sans-serif">
                  Extensions / الإضافات
                </text>
                <text x="24" y="86" fill="#8197c0" font-size="11" font-family="system-ui, sans-serif">
                  Manage installed browser extensions
                </text>

                <!-- Developer Mode Toggle Card (Highlighted) -->
                <g transform="translate(240, 52)">
                  <rect width="195" height="50" rx="6" fill="#162958" stroke="#01b2bb" stroke-width="1.5" stroke-dasharray="4 2" />
                  <text x="14" y="30" fill="#e2e8f0" font-size="12" font-weight="600" font-family="system-ui, sans-serif">
                    Developer mode
                  </text>
                  <!-- Switch Track ON -->
                  <rect x="140" y="16" width="42" height="20" rx="10" fill="#01b2bb" />
                  <!-- Switch Knob Right -->
                  <circle cx="169" cy="26" r="7.5" fill="#ffffff" />
                </g>

                <!-- Instruction pointer arrow -->
                <path d="M 390 115 L 375 90" stroke="#00e5ff" stroke-width="2" stroke-linecap="round" />
                <polygon points="375,90 384,93 378,100" fill="#00e5ff" />
              </svg>
            </div>
          </div>
        {:else}
          <div class="step-content">
            <p class="step-instruction">
              {$t("extension.step2Desc")}
            </p>

            <!-- Inline Illustrative SVG Panel: Load Unpacked Button -->
            <div class="illustrative-panel" aria-hidden="true">
              <svg viewBox="0 0 460 140" class="panel-svg" fill="none" xmlns="http://www.w3.org/2000/svg">
                <!-- Browser Action Bar Background -->
                <rect width="460" height="140" rx="8" fill="#0c193c" stroke="#1d2e5a" stroke-width="1.5" />

                <!-- Action Button 1: Load unpacked (Highlighted) -->
                <g transform="translate(20, 20)">
                  <rect width="145" height="36" rx="6" fill="#01b2bb" fill-opacity="0.2" stroke="#01b2bb" stroke-width="1.8" />
                  <text x="16" y="23" fill="#00e5ff" font-size="12" font-weight="700" font-family="system-ui, sans-serif">
                    Load unpacked
                  </text>
                </g>

                <!-- Action Button 2 & 3: Inactive -->
                <rect x="175" y="20" width="115" height="36" rx="6" fill="#14244d" stroke="#263a6a" stroke-width="1" />
                <text x="190" y="43" fill="#64748b" font-size="11" font-family="system-ui, sans-serif">
                  Pack extension
                </text>

                <rect x="300" y="20" width="85" height="36" rx="6" fill="#14244d" stroke="#263a6a" stroke-width="1" />
                <text x="325" y="43" fill="#64748b" font-size="11" font-family="system-ui, sans-serif">
                  Update
                </text>

                <!-- Folder Selection Box Representation -->
                <g transform="translate(20, 72)">
                  <rect width="420" height="48" rx="6" fill="#112046" stroke="#263a6a" stroke-width="1" />
                  <!-- Folder icon -->
                  <path d="M 18 16 L 30 16 L 36 22 L 54 22 A 2 2 0 0 1 56 24 L 56 36 A 2 2 0 0 1 54 38 L 18 38 A 2 2 0 0 1 16 36 L 16 18 A 2 2 0 0 1 18 16 Z" fill="#f59e0b" opacity="0.9" />
                  <text x="68" y="26" fill="#e2e8f0" font-size="11" font-weight="600" font-family="system-ui, sans-serif">
                    SAYL Browser Integration
                  </text>
                  <text x="68" y="40" fill="#718bb8" font-size="9" font-family="monospace">
                    %LOCALAPPDATA%\Sayl\extension\chromium
                  </text>
                </g>
              </svg>
            </div>

            <!-- Absolute Path Box and Actions -->
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
                      <path d="M6 3a2 2 0 00-2 2v11a2 2 0 002 2h8a2 2 0 002-2V5a2 2 0 00-2-2 3 3 0 01-3 3H9a3 3 0 01-3-3z" />
                    </svg>
                    <span>{$t("extension.copyPath")}</span>
                  {/if}
                </button>
              </div>
            </div>
          </div>
        {/if}

        <!-- Browser Actions Row -->
        <div class="guide-actions-bar">
          <button type="button" class="btn btn-secondary" on:click={handleOpenBrowserExtensions}>
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
              <path fill-rule="evenodd" d="M4.25 2A2.25 2.25 0 002 4.25v11.5A2.25 2.25 0 004.25 18h11.5A2.25 2.25 0 0018 15.75V4.25A2.25 2.25 0 0015.75 2H4.25zm4.03 6.28a.75.75 0 00-1.06-1.06L4.97 9.47a.75.75 0 000 1.06l2.25 2.25a.75.75 0 001.06-1.06L6.81 10.25h6.44a.75.75 0 000-1.5H6.81l1.47-1.47z" clip-rule="evenodd" />
            </svg>
            <span>{$t("extension.openBrowserPage")}</span>
          </button>

          <button type="button" class="btn btn-secondary" on:click={handleOpenFolder}>
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
              <path fill-rule="evenodd" d="M2 4.75C2 3.784 2.784 3 3.75 3h4.836c.464 0 .909.184 1.237.513l1.414 1.414a1.75 1.75 0 001.237.513H16.25c.966 0 1.75.784 1.75 1.75v7.06c0 .966-.784 1.75-1.75 1.75H3.75A1.75 1.75 0 012 14.31V4.75z" clip-rule="evenodd" />
            </svg>
            <span>{$t("extension.openFolder")}</span>
          </button>
        </div>

        <!-- Live Status Hint -->
        <div class="status-return-hint">
          {#if browser?.connected}
            <span class="status-indicator-badge success">
              <span class="pulse-dot"></span>
              {$t("extension.statusConnected")} ✓
            </span>
          {:else}
            <span class="status-indicator-badge checking">
              <span class="spin-dot"></span>
              {$t("extension.returnInstruction")}
            </span>
          {/if}
        </div>
      </div>

      <!-- Dialog Footer -->
      <div class="guide-footer">
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
  .guide-backdrop {
    position: fixed;
    inset: 0;
    background-color: rgba(6, 14, 38, 0.72);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    padding: 1rem;
  }

  .guide-dialog {
    background: #0f1d46;
    border: 1px solid #23386e;
    border-radius: 12px;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.55);
    width: 100%;
    max-width: 540px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    color: #e2e8f0;
  }

  .guide-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    padding: 1.25rem 1.5rem 1rem;
    border-bottom: 1px solid #1c2e5c;
  }

  .guide-badge {
    font-size: 0.75rem;
    font-weight: 700;
    color: #00e5ff;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    margin-bottom: 0.25rem;
    display: block;
  }

  .guide-title {
    font-size: 1.15rem;
    font-weight: 700;
    color: #ffffff;
    margin: 0;
  }

  .guide-close-btn {
    background: transparent;
    border: none;
    color: #8b9fc7;
    cursor: pointer;
    padding: 0.25rem;
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .guide-close-btn:hover {
    background: #192a56;
    color: #ffffff;
  }

  .guide-steps-nav {
    display: flex;
    border-bottom: 1px solid #1c2e5c;
    background: #0b173b;
  }

  .step-tab {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    color: #8197c0;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 150ms ease;
  }

  .step-tab:hover {
    color: #cad8f3;
  }

  .step-tab.active {
    color: #00e5ff;
    border-bottom-color: #00e5ff;
    background: #0f1d46;
  }

  .step-num {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: #1a2c5a;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 0.75rem;
  }

  .step-tab.active .step-num {
    background: #01b2bb;
    color: #ffffff;
  }

  .guide-body {
    padding: 1.25rem 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .step-instruction {
    font-size: 0.95rem;
    line-height: 1.55;
    color: #d1dff8;
    margin: 0 0 1rem;
  }

  .illustrative-panel {
    width: 100%;
    border-radius: 8px;
    overflow: hidden;
    background: #0c193c;
  }

  .panel-svg {
    width: 100%;
    height: auto;
    display: block;
  }

  .path-display-box {
    margin-top: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .path-label {
    font-size: 0.8rem;
    font-weight: 600;
    color: #8ea5d0;
  }

  .path-input-group {
    display: flex;
    gap: 0.5rem;
  }

  .path-input {
    flex: 1;
    background: #08112c;
    border: 1px solid #23396c;
    border-radius: 6px;
    color: #38bdf8;
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
    transition: all 150ms ease;
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
    background: #0b1535;
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
    border: 2px solid #00e5ff;
    border-top-color: transparent;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .guide-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.75rem;
    padding: 1rem 1.5rem;
    background: #0b173b;
    border-top: 1px solid #1c2e5c;
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
    background: #01b2bb;
    color: #ffffff;
  }

  .btn-primary:hover {
    background: #0299a1;
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

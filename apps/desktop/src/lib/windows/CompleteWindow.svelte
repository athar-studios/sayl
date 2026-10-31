<script>
  /**
   * IDM-style "Download complete" window (window c).
   *
   * Shows the file name, size and saved path with «فتح» / «فتح المجلد» /
   * «إغلاق», plus «لا تعرض هذه النافذة مرة أخرى» which dispatches
   * `dontShowAgain` and persists `show_download_complete_window`.
   *
   * Every action dismisses the window: the checkbox is persisted first, then the
   * action event is emitted (the payload is always the task id string, matching
   * the `{ id }` argument the Rust commands take) and `close` follows, so the
   * host removes the window deterministically.
   */
  import { createEventDispatcher } from "svelte";
  import { t, currentLang } from "../i18n.js";
  import { DEFAULT_SETTINGS, normalizeSettings } from "../settings.js";
  import { buildCompleteDetails } from "../downloadWindowUi.js";
  import iconSvg from "../../assets/icon.svg";

  export let task;
  /**
   * Persist «لا تعرض هذه النافذة مرة أخرى» from inside the window. Default on,
   * because the in-app floating host (App.svelte) does not listen for
   * `dontShowAgain`; a host that persists it itself can pass `false`.
   */
  export let persistDontShowAgain = true;

  const dispatch = createEventDispatcher();
  let dontShowAgain = false;
  let busy = false;

  $: isRtl = $currentLang === "ar";
  $: details = buildCompleteDetails(task, $currentLang);

  /**
   * Writes `show_download_complete_window = false`. `set_settings` replaces the
   * whole Settings struct, so the current snapshot is read and normalised first —
   * a partial payload would reset every other key to its default.
   */
  async function persistDontShowAgainSetting() {
    if (!persistDontShowAgain) return;
    if (typeof window === "undefined") return;
    if (!(window.__TAURI_INTERNALS__ || window.__TAURI__)) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const current = await invoke("get_settings");
      const next = normalizeSettings({
        ...(current && typeof current === "object" ? current : DEFAULT_SETTINGS),
        show_download_complete_window: false,
      });
      await invoke("set_settings", { settings: next });
    } catch {
      // Best effort: a failed preference write must never keep the window open.
    }
  }

  /**
   * @param {"openFile"|"reveal"|null} action
   */
  async function dismiss(action) {
    if (busy) return;
    busy = true;
    const id = task?.id;
    if (dontShowAgain) {
      await persistDontShowAgainSetting();
      dispatch("dontShowAgain", true);
    }
    if (action) dispatch(action, id);
    dispatch("close", id);
  }
</script>

<div
  class="complete-window"
  class:rtl={isRtl}
  dir={isRtl ? "rtl" : "ltr"}
  role="dialog"
  aria-label={$t("completeWindow.title")}
>
  <div class="title-row">
    <img src={iconSvg} alt="SAYL" class="brand-logo" aria-hidden="true" />
    <span class="heading">{$t("completeWindow.title")}</span>
  </div>

  <div class="details">
    <div class="detail-row">
      <span class="label">{$t("completeWindow.file")}:</span>
      <span class="value" title={details.filename}>
        <bdi dir="auto">{details.filename}</bdi>
      </span>
    </div>
    <div class="detail-row">
      <span class="label">{$t("completeWindow.size")}:</span>
      <span class="value" dir="ltr"><bdi dir="ltr">{details.size}</bdi></span>
    </div>
    <div class="detail-row">
      <span class="label">{$t("completeWindow.savedPath")}:</span>
      <span class="value path" title={details.savedPath}>
        <bdi dir="auto">{details.savedPath}</bdi>
      </span>
    </div>
  </div>

  <label class="option-label">
    <input
      type="checkbox"
      class="option-checkbox"
      bind:checked={dontShowAgain}
      disabled={busy}
    />
    <span>{$t("completeWindow.dontShowAgain")}</span>
  </label>

  <div class="actions">
    <button
      class="btn btn-primary"
      type="button"
      disabled={busy}
      on:click={() => dismiss("openFile")}
    >
      {$t("completeWindow.openFile")}
    </button>
    <button
      class="btn btn-secondary"
      type="button"
      disabled={busy}
      on:click={() => dismiss("reveal")}
    >
      {$t("completeWindow.openFolder")}
    </button>
    <button class="btn btn-secondary" type="button" disabled={busy} on:click={() => dismiss(null)}>
      {$t("completeWindow.close")}
    </button>
  </div>
</div>

<style>
  /* Navy #0b1a4e and turquoise #00b6bd are accents only (primary button, focus,
     checkboxes). Every text/surface/border colour comes from the app theme
     variables shared with the main window, so light and dark both stay legible. */
  .complete-window {
    box-sizing: border-box;
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 14px;
    background: var(--color-bg-card);
    color: var(--color-text);
    font-family: var(--font-family);
    font-size: 0.8125rem;
    user-select: none;
    overflow: hidden;
    pointer-events: auto;
  }

  .complete-window.rtl {
    text-align: right;
  }

  .title-row {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .brand-logo {
    width: 18px;
    height: 18px;
    object-fit: contain;
  }

  .heading {
    font-weight: 700;
    color: var(--color-text);
    font-size: 0.9375rem;
  }

  .details {
    display: flex;
    flex-direction: column;
    gap: 5px;
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  .detail-row {
    display: flex;
    align-items: baseline;
    gap: 5px;
    min-width: 0;
    line-height: 1.4;
  }

  /* Labels muted, values normal text colour. */
  .label {
    color: var(--color-text-muted);
    font-weight: 600;
    flex-shrink: 0;
    white-space: nowrap;
  }

  .value {
    color: var(--color-text);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    flex: 1;
  }

  .value.path {
    white-space: normal;
    word-break: break-all;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    font-weight: 500;
  }

  .option-label {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 0.75rem;
    color: var(--color-text-muted);
    cursor: pointer;
    flex-shrink: 0;
  }

  .option-checkbox {
    width: 14px;
    height: 14px;
    cursor: pointer;
    accent-color: var(--color-accent);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .btn {
    flex: 1;
    height: 30px;
    padding: 0 12px;
    border-radius: 4px;
    font-family: inherit;
    font-size: 0.8125rem;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid var(--color-border);
    transition: background-color 120ms ease;
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

  .btn-secondary {
    background: var(--color-bg-card);
    color: var(--color-text);
    border-color: var(--color-text-subtle);
  }

  .btn-secondary:hover:not(:disabled) {
    background: var(--color-bg-card-hover);
    border-color: var(--color-text-subtle);
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
</style>

<script>
  import { t, currentLang } from "./i18n.js";
  import { formatBytes } from "./format.js";
  import { createEventDispatcher } from "svelte";
  import iconSvg from "../assets/icon.svg";

  export let task;

  const dispatch = createEventDispatcher();

  $: isRtl = $currentLang === "ar";
  $: sizeLabel =
    task.total_size && task.total_size > 0
      ? formatBytes(task.total_size, $currentLang)
      : formatBytes(task.written || 0, $currentLang);
  $: folderPath = task.folder || "—";
</script>

<div
  class="complete-window"
  class:rtl={isRtl}
  dir={isRtl ? "rtl" : "ltr"}
  role="dialog"
  aria-label={$t("completeWindow.title")}
>
  <div class="window-header">
    <div class="brand-wrap">
      <div class="logo-box" aria-hidden="true">
        <img src={iconSvg} alt="SAYL" class="logo-img" />
      </div>
      <span class="heading">{$t("completeWindow.title")}</span>
    </div>
  </div>

  <div class="details">
    <div class="detail-row">
      <span class="label">{$t("completeWindow.file")}:</span>
      <span class="value" title={task.filename}>
        <bdi dir="auto">{task.filename || "—"}</bdi>
      </span>
    </div>
    <div class="detail-row">
      <span class="label">{$t("completeWindow.size")}:</span>
      <span class="value"><bdi dir="ltr">{sizeLabel}</bdi></span>
    </div>
    <div class="detail-row folder-row">
      <span class="label">{$t("completeWindow.folder")}:</span>
      <span class="value folder" title={folderPath}>
        <bdi dir="auto">{folderPath}</bdi>
      </span>
    </div>
  </div>

  <div class="actions">
    <button class="action-btn btn-primary" type="button" on:click={() => dispatch("openFile", task.id)}>
      {$t("completeWindow.openFile")}
    </button>
    <button class="action-btn btn-secondary" type="button" on:click={() => dispatch("reveal", task.id)}>
      {$t("completeWindow.openFolder")}
    </button>
    <button class="action-btn btn-muted" type="button" on:click={() => dispatch("close", task.id)}>
      {$t("completeWindow.close")}
    </button>
  </div>
</div>

<style>
  .complete-window {
    width: 100%;
    height: 100%;
    box-sizing: border-box;
    padding: 1rem 1.1rem;
    background: var(--color-bg-card, #ffffff);
    color: var(--color-text, #1f2937);
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    font-family: var(--font-family, "Segoe UI", Arial, sans-serif);
    user-select: none;
    overflow: hidden;
  }

  .complete-window.rtl {
    text-align: right;
  }

  :global([data-theme="dark"]) .complete-window {
    background: #1f2937;
    color: #f3f4f6;
  }

  .window-header {
    flex-shrink: 0;
  }

  .brand-wrap {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .logo-box {
    width: 22px;
    height: 22px;
    flex-shrink: 0;
  }

  .logo-img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .heading {
    font-size: 1rem;
    font-weight: 700;
    color: #16a34a;
  }

  :global([data-theme="dark"]) .heading {
    color: #4ade80;
  }

  .details {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
    flex: 1;
    min-height: 0;
  }

  .detail-row {
    display: flex;
    align-items: baseline;
    gap: 0.35rem;
    font-size: 0.8125rem;
    line-height: 1.35;
    min-width: 0;
  }

  .complete-window.rtl .detail-row {
    flex-direction: row;
  }

  .label {
    color: #4b5563;
    flex-shrink: 0;
  }

  :global([data-theme="dark"]) .label {
    color: #9ca3af;
  }

  .value {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    flex: 1;
  }

  .folder-row .value.folder {
    white-space: normal;
    word-break: break-all;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .actions {
    display: flex;
    gap: 0.5rem;
    flex-shrink: 0;
  }

  .complete-window.rtl .actions {
    flex-direction: row-reverse;
  }

  .action-btn {
    flex: 1;
    height: 34px;
    font-size: 0.8125rem;
    font-weight: 600;
    border-radius: 6px;
    border: 1px solid transparent;
    cursor: pointer;
  }

  .btn-primary {
    background: #00b6bd;
    color: #fff;
    border-color: #009ea4;
  }

  .btn-secondary {
    background: #f3f4f6;
    color: #1f2937;
    border-color: #d1d5db;
  }

  :global([data-theme="dark"]) .btn-secondary {
    background: #374151;
    color: #f3f4f6;
    border-color: #4b5563;
  }

  .btn-muted {
    background: transparent;
    color: #4b5563;
    border-color: #d1d5db;
  }

  :global([data-theme="dark"]) .btn-muted {
    color: #d1d5db;
    border-color: #4b5563;
  }
</style>

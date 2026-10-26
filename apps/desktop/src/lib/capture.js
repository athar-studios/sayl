/**
 * Browser capture → desktop prepare_capture_task contract (Tauri camelCase args).
 */

import { classifyFile } from "./classify.js";
import { resolveTranslation } from "./i18n.js";

/** Subfolder names under download_dir (matches Settings categories copy). */
export const CATEGORY_SUBFOLDERS = {
  video: "Video",
  audio: "Audio",
  documents: "Documents",
  programs: "Programs",
  compressed: "Compressed",
  other: "Other",
};

/**
 * @param {string} downloadDir
 * @param {string} [categoryId]
 * @param {string} [filename]
 * @returns {string}
 */
export function defaultSavePathForCapture(downloadDir, categoryId, filename) {
  const base = (downloadDir || "").trim();
  if (!base) return "";

  const cat =
    categoryId && CATEGORY_SUBFOLDERS[categoryId]
      ? categoryId
      : classifyFile(filename || "", null);

  const sub = CATEGORY_SUBFOLDERS[cat] || CATEGORY_SUBFOLDERS.other;
  const sep = base.includes("\\") ? "\\" : "/";
  const trimmed = base.replace(/[/\\]+$/, "");
  return `${trimmed}${sep}${sub}`;
}

/**
 * Resolves savePath: explicit folder from dialog, else category/default download dir.
 * @param {{ download_dir?: string }} settings
 * @param {string|undefined} folderFromDialog
 * @param {string|undefined} categoryId
 * @param {string|undefined} filename
 * @returns {string}
 */
export function resolveCaptureSavePath(settings, folderFromDialog, categoryId, filename) {
  const trimmedFolder = (folderFromDialog || "").trim();
  if (trimmedFolder) return trimmedFolder;
  return defaultSavePathForCapture(settings?.download_dir || "", categoryId, filename);
}

/**
 * Exact payload shape sent to Tauri `prepare_capture_task`.
 * @param {{ id: string, filename: string, folder?: string, category?: string, settings?: { download_dir?: string } }} input
 */
export function buildPrepareCaptureTaskArgs(input) {
  const id = input?.id;
  const filename = (input?.filename || "").trim();
  const savePath = resolveCaptureSavePath(
    input?.settings || {},
    input?.folder,
    input?.category,
    filename
  );

  return {
    id,
    filename,
    savePath,
  };
}

/**
 * User-facing Arabic/English message; log raw error separately for diagnostics.
 * @param {unknown} err
 * @param {string} [lang="ar"]
 * @returns {string}
 */
export function friendlyPrepareCaptureError(err, lang = "ar") {
  const raw = typeof err === "string" ? err : err?.message || String(err);
  const lower = raw.toLowerCase();

  if (lower.includes("savepath") || lower.includes("save_path") || lower.includes("download_dir")) {
    return resolveTranslation(lang, "capture.invalidFolder");
  }
  if (lower.includes("invalid task id")) {
    return resolveTranslation(lang, "capture.taskNotFound");
  }
  if (lower.includes("permission") || lower.includes("access is denied")) {
    return resolveTranslation(lang, "capture.folderAccessDenied");
  }

  return resolveTranslation(lang, "capture.prepareFailed");
}

/** @param {unknown} err */
export function logCaptureDiagnostic(err, context = "prepare_capture_task") {
  const raw = typeof err === "string" ? err : err?.message || String(err);
  if (typeof console !== "undefined" && console.error) {
    console.error(`[sayl-capture] ${context}:`, raw);
  }
}

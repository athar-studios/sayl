import { resolveTranslation } from "./i18n.js";

/**
 * Formats a raw byte count into human-readable string with units.
 */
export function formatBytes(bytes, lang = "ar") {
  if (bytes === null || bytes === undefined || isNaN(bytes)) {
    return resolveTranslation(lang, "task.unknownSize");
  }
  const b = Number(bytes);
  if (b < 0) return "0 " + resolveTranslation(lang, "units.b");
  if (b === 0) return "0 " + resolveTranslation(lang, "units.b");

  const k = 1024;
  const sizes = ["units.b", "units.kb", "units.mb", "units.gb", "units.tb"];
  const i = Math.min(Math.floor(Math.log(b) / Math.log(k)), sizes.length - 1);
  const val = b / Math.pow(k, i);
  const formatted = i === 0 ? Math.round(val) : val.toFixed(1);

  return `${formatted} ${resolveTranslation(lang, sizes[i])}`;
}

/**
 * Formats transfer speed in bytes per second.
 */
export function formatSpeed(bytesPerSec, lang = "ar") {
  if (!bytesPerSec || bytesPerSec <= 0) {
    return `0 ${resolveTranslation(lang, "units.bps")}`;
  }
  const k = 1024;
  const sizes = ["units.bps", "units.kbps", "units.mbps", "units.gbps"];
  const i = Math.min(Math.floor(Math.log(bytesPerSec) / Math.log(k)), sizes.length - 1);
  const val = bytesPerSec / Math.pow(k, i);
  const formatted = i === 0 ? Math.round(val) : val.toFixed(1);

  return `${formatted} ${resolveTranslation(lang, sizes[i])}`;
}

/**
 * Formats ETA remaining seconds into compact human-readable duration.
 */
export function formatEta(seconds, lang = "ar") {
  if (seconds === null || seconds === undefined || isNaN(seconds) || seconds <= 0) {
    return resolveTranslation(lang, "task.calculating");
  }

  const s = Math.round(seconds);
  const secUnit = resolveTranslation(lang, "units.seconds");
  const minUnit = resolveTranslation(lang, "units.minutes");
  const hrUnit = resolveTranslation(lang, "units.hours");
  const dayUnit = resolveTranslation(lang, "units.days");

  if (s < 60) {
    return `${s} ${secUnit}`;
  }
  const m = Math.floor(s / 60);
  const remS = s % 60;
  if (m < 60) {
    return remS > 0 ? `${m} ${minUnit} ${remS} ${secUnit}` : `${m} ${minUnit}`;
  }
  const h = Math.floor(m / 60);
  const remM = m % 60;
  if (h < 24) {
    return remM > 0 ? `${h} ${hrUnit} ${remM} ${minUnit}` : `${h} ${hrUnit}`;
  }
  const d = Math.floor(h / 24);
  const remH = h % 24;
  return remH > 0 ? `${d} ${dayUnit} ${remH} ${hrUnit}` : `${d} ${dayUnit}`;
}

/**
 * Returns true if the task state allows pausing.
 */
export function canPause(state) {
  return state === "Probing" || state === "Downloading";
}

/**
 * Returns true if the task state allows resuming.
 */
export function canResume(state) {
  return state === "Paused" || state === "Queued" || state === "Failed";
}

/**
 * Returns true if the task state allows cancellation.
 */
export function canCancel(state) {
  return state !== "Completed" && state !== "Cancelled";
}

/**
 * Returns true if the task state is terminal.
 */
export function isTerminal(state) {
  return state === "Completed" || state === "Cancelled" || state === "Failed";
}

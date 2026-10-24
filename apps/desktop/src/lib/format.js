import { resolveTranslation } from "./i18n.js";

/**
 * Formats a raw byte count into human-readable string with units.
 * Arabic units use standard Arabic transliterated units: جيجا, ميجا, كيلو, بايت.
 */
export function formatBytes(bytes, lang = "ar") {
  if (bytes === null || bytes === undefined || isNaN(bytes)) {
    return resolveTranslation(lang, "task.unknownSize");
  }
  const b = Number(bytes);
  if (b <= 0) return "0 " + resolveTranslation(lang, "units.b");

  const k = 1024;
  const sizes = ["units.b", "units.kb", "units.mb", "units.gb", "units.tb"];
  const i = Math.min(Math.floor(Math.log(b) / Math.log(k)), sizes.length - 1);
  const val = b / Math.pow(k, i);
  const formatted = i === 0 ? Math.round(val) : val.toFixed(1);

  return `${formatted} ${resolveTranslation(lang, sizes[i])}`;
}

/**
 * Formats transfer speed in bytes per second.
 * Arabic units use ميجا/ث, كيلو/ث, جيجا/ث, بايت/ث.
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
 * Formats percentage with correct bidi and Arabic percent symbol (٪).
 * No parentheses, rendered cleanly as e.g. "52.5٪".
 */
export function formatPercent(percent, lang = "ar") {
  if (percent === null || percent === undefined || isNaN(percent)) {
    return "";
  }
  const p = Number(percent);
  const clamped = Math.min(100, Math.max(0, p));
  const formatted = Number.isInteger(clamped) ? clamped.toString() : clamped.toFixed(1);

  if (lang === "ar") {
    return `${formatted}٪`;
  }
  return `${formatted}%`;
}

/**
 * Formats ETA remaining seconds into compact human-readable duration.
 * In Arabic: compact letters like "3د 20ث", "45ث", "1س 1د".
 * In English: "3 m 20 s", "45 s", "1 h 1 m".
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

  if (lang === "ar") {
    if (s < 60) {
      return `${s}${secUnit}`;
    }
    const m = Math.floor(s / 60);
    const remS = s % 60;
    if (m < 60) {
      return remS > 0 ? `${m}${minUnit} ${remS}${secUnit}` : `${m}${minUnit}`;
    }
    const h = Math.floor(m / 60);
    const remM = m % 60;
    if (h < 24) {
      return remM > 0 ? `${h}${hrUnit} ${remM}${minUnit}` : `${h}${hrUnit}`;
    }
    const d = Math.floor(h / 24);
    const remH = h % 24;
    return remH > 0 ? `${d} ${dayUnit} ${remH}${hrUnit}` : `${d} ${dayUnit}`;
  }

  // English formatting
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
 * Wraps text in First Strong Isolate (U+2068) and Pop Directional Isolate (U+2069)
 * for bidi safety when embedding numbers, URLs or mixed scripts in RTL contexts.
 */
export function formatBidi(str) {
  if (str === null || str === undefined || str === "") return "";
  return `\u2068${str}\u2069`;
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

import { resolveTranslation } from "./i18n.js";

/**
 * Formats a raw byte count into human-readable string with units.
 * Uses standard units (B, KB, MB, GB, TB) with strict LTR isolation.
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
 * Uses standard units (B/s, KB/s, MB/s, GB/s) with strict LTR isolation.
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
 * Formats percentage with standard % symbol and no parentheses.
 * e.g. "52.5%".
 */
export function formatPercent(percent, lang = "ar") {
  if (percent === null || percent === undefined || isNaN(percent)) {
    return "";
  }
  const p = Number(percent);
  const clamped = Math.min(100, Math.max(0, p));
  const formatted = Number.isInteger(clamped) ? clamped.toString() : clamped.toFixed(1);

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
 * Wraps text in Left-to-Right Isolate (U+2066) and Pop Directional Isolate (U+2069)
 * to strictly enforce LTR ordering in RTL contexts.
 */
export function formatLtr(str) {
  if (str === null || str === undefined || str === "") return "";
  return `\u2066${str}\u2069`;
}

/**
 * Formats an ISO 8601 date string into human-friendly localized date/time:
 * - If date is today: "اليوم hh:mm م" (ar) / "Today h:mm A" (en)
 * - If date is yesterday: "أمس hh:mm م" (ar) / "Yesterday h:mm A" (en)
 * - Otherwise: "D MMMM YYYY" (e.g. "25 أكتوبر 2026" / "Oct 25, 2026")
 * Falls back to "—" if string is absent, empty, or invalid.
 */
export function formatDate(isoString, lang = "ar", now = new Date()) {
  if (
    !isoString ||
    (typeof isoString !== "string" &&
      typeof isoString !== "number" &&
      !(isoString instanceof Date))
  ) {
    return "—";
  }
  const date = new Date(isoString);
  if (isNaN(date.getTime())) return "—";

  try {
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const targetDay = new Date(date.getFullYear(), date.getMonth(), date.getDate());
    const diffTime = today.getTime() - targetDay.getTime();
    const diffDays = Math.round(diffTime / (1000 * 60 * 60 * 24));

    if (diffDays === 0) {
      if (lang === "ar") {
        const time = new Intl.DateTimeFormat("ar-EG", {
          hour: "2-digit",
          minute: "2-digit",
          hour12: true,
          numberingSystem: "latn",
        }).format(date);
        return `اليوم ${time}`;
      } else {
        const time = new Intl.DateTimeFormat("en-US", {
          hour: "numeric",
          minute: "2-digit",
          hour12: true,
        }).format(date);
        return `Today ${time}`;
      }
    }

    if (diffDays === 1) {
      if (lang === "ar") {
        const time = new Intl.DateTimeFormat("ar-EG", {
          hour: "2-digit",
          minute: "2-digit",
          hour12: true,
          numberingSystem: "latn",
        }).format(date);
        return `أمس ${time}`;
      } else {
        const time = new Intl.DateTimeFormat("en-US", {
          hour: "numeric",
          minute: "2-digit",
          hour12: true,
        }).format(date);
        return `Yesterday ${time}`;
      }
    }

    if (lang === "ar") {
      return new Intl.DateTimeFormat("ar-EG", {
        day: "numeric",
        month: "long",
        year: "numeric",
        numberingSystem: "latn",
      }).format(date);
    } else {
      return new Intl.DateTimeFormat("en-US", {
        day: "numeric",
        month: "short",
        year: "numeric",
      }).format(date);
    }
  } catch {
    return "—";
  }
}

/**
 * Returns true if the task state allows pausing.
 */
export function canPause(state) {
  return state === "Probing" || state === "Downloading";
}

/**
 * Returns true if task state allows pausing per toolbar and table enablement contract.
 * Pause only for Queued, Probing, Downloading, Verifying, Finalizing.
 */
export function canPauseTask(state) {
  return (
    state === "Queued" ||
    state === "Probing" ||
    state === "Downloading" ||
    state === "Verifying" ||
    state === "Finalizing"
  );
}

/**
 * Returns true if the task state allows resuming.
 */
export function canResume(state) {
  return state === "Paused" || state === "Queued" || state === "Failed";
}

/**
 * Returns true if task state allows resuming per toolbar and table enablement contract.
 * Resume only for Paused, Pausing, Failed, Queued, Cancelled.
 */
export function canResumeTask(state) {
  return (
    state === "Paused" ||
    state === "Pausing" ||
    state === "Failed" ||
    state === "Queued" ||
    state === "Cancelled"
  );
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

/**
 * Returns true if a task is currently in pending start/resume state.
 * Supports pendingStart being a Set, an Array, or an Object/map.
 */
export function isTaskPending(pendingStart, taskId) {
  if (!pendingStart || !taskId) return false;
  if (pendingStart instanceof Set) {
    return pendingStart.has(taskId);
  }
  if (Array.isArray(pendingStart)) {
    return pendingStart.includes(taskId);
  }
  return Boolean(pendingStart[taskId]);
}

/**
 * Returns the effective translation key for a task's state or connecting label.
 */
export function getTaskStateKey(state, isPending = false) {
  return isPending ? "task.connecting" : `state.${state}`;
}

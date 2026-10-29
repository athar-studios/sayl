/**
 * Pure, DOM-free helpers backing the IDM-style download window.
 *
 * Everything in this module is unit-testable: no Svelte imports, no timers that
 * cannot be injected, no Tauri calls. `DownloadWindow.svelte` and
 * `StandaloneDownloadWindow.svelte` are the only consumers.
 */

import { resolveTranslation } from "./i18n.js";
import {
  formatBytes,
  formatSpeed,
  formatEta,
  formatPercent,
  canPauseTask,
  canResumeTask,
  canCancel,
} from "./format.js";

import {
  mergeTaskMonotonic,
  mergeTasksMonotonic,
  isProgressResetState,
} from "./monotonic.js";

export { mergeTaskMonotonic, mergeTasksMonotonic, isProgressResetState };

const MIN_SPEED_LIMIT_KBPS = 1;
// No user-facing cap; values saturate at u32::MAX only because the backend stores KB/s as u32.
const MAX_SPEED_LIMIT_KBPS = 4_294_967_295;
const KBPS_PER_MBPS = 1024;

/** Coerces anything to a finite number, or `null` when it is not numeric. */
function toFiniteNumber(value) {
  if (value === null || value === undefined || value === "") return null;
  if (typeof value === "boolean") return null;
  const n = typeof value === "number" ? value : Number(value);
  return Number.isFinite(n) ? n : null;
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

function clamp01(value) {
  const n = toFiniteNumber(value);
  if (n === null) return 0;
  return clamp(n, 0, 1);
}

/** Renders a percent number without a trailing `.0` and without the `%` sign. */
function percentLabel(percent) {
  const rounded = Math.round(percent * 10) / 10;
  return Number.isInteger(rounded) ? String(rounded) : rounded.toFixed(1);
}

/**
 * Builds the header title in the IDM style: `12% file.zip`.
 * A missing/invalid percent falls back to the bare filename; a missing filename
 * falls back to the bare percent. Both missing yields an empty string.
 *
 * @param {number|null|undefined} percent
 * @param {string|null|undefined} filename
 * @returns {string}
 */
export function buildWindowTitle(percent, filename) {
  const name = typeof filename === "string" ? filename.trim() : "";
  const pct = toFiniteNumber(percent);
  const label = pct === null ? "" : `${percentLabel(clamp(pct, 0, 100))}%`;

  if (!label) return name;
  if (!name) return label;
  return `${label} ${name}`;
}

/**
 * Shortens `text` in the middle so both the start (scheme/host) and the tail
 * (file name) stay readable. Text that already fits is returned unchanged.
 *
 * @param {string|null|undefined} text
 * @param {number} [maxLen=40] maximum length of the returned string
 * @returns {string}
 */
export function truncateMiddle(text, maxLen = 40) {
  const str = text === null || text === undefined ? "" : String(text);
  const limit = Math.floor(toFiniteNumber(maxLen) ?? 40);

  if (limit <= 0) return "";
  if (str.length <= limit) return str;
  if (limit === 1) return "…";
  if (limit === 2) return `${str.slice(0, 1)}…`;

  const tail = Math.ceil((limit - 1) / 2);
  const head = limit - 1 - tail;
  return `${str.slice(0, head)}…${str.slice(str.length - tail)}`;
}

/**
 * Fixed-size moving average used to smooth the displayed transfer speed.
 * Non-finite and negative samples are ignored. Returns `0` before the first
 * accepted sample.
 *
 * @param {{ size?: number }} [options]
 * @returns {{ push: (value: unknown) => number, value: () => number }}
 */
export function createMovingAverage({ size = 5 } = {}) {
  const max = Math.max(1, Math.floor(toFiniteNumber(size) ?? 5));
  /** @type {number[]} */
  const samples = [];
  let total = 0;

  function current() {
    return samples.length > 0 ? total / samples.length : 0;
  }

  return {
    push(value) {
      const n = toFiniteNumber(value);
      if (n === null || n < 0) return current();

      samples.push(n);
      total += n;
      while (samples.length > max) {
        total -= samples.shift();
      }
      return current();
    },
    value() {
      return current();
    },
  };
}

function percentOf(written, total) {
  const w = toFiniteNumber(written);
  const t = toFiniteNumber(total);
  if (w === null || t === null || t <= 0) return null;
  return clamp(Math.round((clamp(w, 0, t) / t) * 1000) / 10, 0, 100);
}

/**
 * Throttles task snapshots to at most one render per `intervalMs`.
 *
 * `push(task)` always stores the newest snapshot and returns the snapshot that
 * should currently be rendered — the freshly emitted one when the interval has
 * elapsed, otherwise the previously emitted view (so callers never see `null`
 * flapping). `flush()` forces an emit of the latest snapshot.
 *
 * The emitted view never reports a `written` value lower than a previously
 * emitted view for the same task id.
 *
 * @param {{ intervalMs?: number, now?: () => number }} [options]
 */
export function createThrottledTaskView({ intervalMs = 500, now = () => Date.now() } = {}) {
  const interval = Math.max(0, toFiniteNumber(intervalMs) ?? 500);
  const clock = typeof now === "function" ? now : () => Date.now();

  /** @type {object|null} */
  let latestTask = null;
  /** @type {object|null} */
  let emittedView = null;
  /** @type {number|null} */
  let lastEmitAt = null;
  /** @type {string|null} */
  let lastTaskId = null;
  let lastWritten = -1;

  function buildView(task) {
    const written = Math.max(0, toFiniteNumber(task.written) ?? 0);
    const monotonicWritten = Math.max(written, lastWritten);
    lastWritten = monotonicWritten;

    return {
      ...task,
      written: monotonicWritten,
      percent: percentOf(monotonicWritten, task.total_size),
    };
  }

  function emit(task) {
    emittedView = buildView(task);
    lastEmitAt = clock();
    lastTaskId = task.id ?? null;
    return emittedView;
  }

  function reset() {
    latestTask = null;
    emittedView = null;
    lastEmitAt = null;
    lastTaskId = null;
    lastWritten = -1;
  }

  return {
    push(task) {
      if (!task || typeof task !== "object") return emittedView;

      // A different task id restarts the monotonic guard and emits immediately.
      if ((task.id ?? null) !== lastTaskId) {
        reset();
        latestTask = task;
        return emit(task);
      }

      latestTask = task;
      const currentTime = clock();
      if (lastEmitAt !== null && currentTime - lastEmitAt < interval) {
        return emittedView;
      }
      return emit(task);
    },
    flush() {
      if (!latestTask) return emittedView;
      return emit(latestTask);
    },
    reset,
  };
}

function normalizeSegment(segment, index) {
  const raw = segment && typeof segment === "object" ? segment : {};
  const startRaw = toFiniteNumber(raw.start);
  const endRaw = toFiniteNumber(raw.end);
  const start = startRaw === null ? 0 : Math.max(0, Math.floor(startRaw));
  const end = endRaw === null ? start : Math.max(start, Math.floor(endRaw));
  const span = end - start;

  let fraction;
  if (raw.fraction !== undefined && toFiniteNumber(raw.fraction) !== null) {
    fraction = raw.fraction;
  } else if (toFiniteNumber(raw.written) !== null && span > 0) {
    fraction = (toFiniteNumber(raw.written) ?? 0) / span;
  } else {
    fraction = raw.total_completed === false ? 0 : 1;
  }

  return {
    index: index + 1,
    start,
    end,
    span,
    fraction: clamp01(fraction),
  };
}

function weightedFraction(group) {
  const span = group.reduce((sum, seg) => sum + Math.max(seg.span, 0), 0);
  if (span <= 0) {
    const avg = group.reduce((sum, seg) => sum + seg.fraction, 0) / group.length;
    return clamp01(avg);
  }
  const covered = group.reduce((sum, seg) => sum + seg.fraction * Math.max(seg.span, 0), 0);
  return clamp01(covered / span);
}

/** Splits `list` into at most `count` contiguous groups of near-equal size. */
function mergeAdjacent(list, count) {
  const groups = Math.min(count, list.length);
  const size = list.length / groups;
  /** @type {Array<Array<object>>} */
  const out = [];
  for (let i = 0; i < groups; i += 1) {
    const from = Math.floor(i * size);
    const to = i === groups - 1 ? list.length : Math.floor((i + 1) * size);
    out.push(list.slice(from, Math.max(to, from + 1)));
  }
  return out;
}

/**
 * Builds the IDM-style connection strip: one cell per segment/connection,
 * merged down to `maxCells` adjacent cells when there are more connections
 * than cells, with every `fraction` clamped to `0..1`.
 *
 * When `totalSize` is known the file is binned into `min(maxCells, segments)`
 * equal byte bands, so pending areas render as empty cells while completed
 * ranges fill in (checkpoints only persist completed ranges).
 *
 * @param {Array<{start?: number, end?: number, written?: number, fraction?: number, total_completed?: boolean}>} segments
 * @param {{ maxCells?: number, totalSize?: number|null }} [options]
 * @returns {Array<{ key: string, label: string, fraction: number, start: number, end: number }>}
 */
export function buildSegmentBars(segments, { maxCells = 16, totalSize = null } = {}) {
  const list = (Array.isArray(segments) ? segments : []).map(normalizeSegment);
  if (list.length === 0) return [];

  const limit = Math.max(1, Math.floor(toFiniteNumber(maxCells) ?? 16));
  const total = toFiniteNumber(totalSize);

  if (total !== null && total > 0) {
    const cellCount = limit;
    const bandSize = total / cellCount;
    /** @type {Array<{ key: string, label: string, fraction: number, start: number, end: number }>} */
    const cells = [];

    for (let i = 0; i < cellCount; i += 1) {
      const start = i === 0 ? 0 : Math.round(i * bandSize);
      const end = i === cellCount - 1 ? total : Math.round((i + 1) * bandSize);
      const span = Math.max(end - start, 1);

      let covered = 0;
      for (const seg of list) {
        const overlap = Math.min(end, seg.start + seg.span) - Math.max(start, seg.start);
        if (overlap > 0) covered += overlap * seg.fraction;
      }

      cells.push({
        key: `cell-${i}`,
        label: String(i + 1),
        fraction: clamp01(covered / span),
        start,
        end,
      });
    }
    return cells;
  }

  return mergeAdjacent(list, limit).map((group, i) => ({
    key: `cell-${i}`,
    label: String(i + 1),
    fraction: weightedFraction(group),
    start: group[0].start,
    end: group[group.length - 1].end,
  }));
}

/**
 * Rows for the «عرض التفاصيل» table: downloaded bytes + a coarse status per
 * segment range. Status is one of `done` | `downloading` | `pending`.
 *
 * @param {Array<{start?: number, end?: number, written?: number, total_completed?: boolean}>} segments
 */
export function buildSegmentDetailRows(segments) {
  return (Array.isArray(segments) ? segments : []).map((segment, index) => {
    const raw = segment && typeof segment === "object" ? segment : {};
    const start = Math.max(0, Math.floor(toFiniteNumber(raw.start) ?? 0));
    const end = Math.max(start, Math.floor(toFiniteNumber(raw.end) ?? start));
    const span = end - start;
    const writtenRaw = toFiniteNumber(raw.written);
    const written =
      writtenRaw === null ? (raw.total_completed === false ? 0 : span) : clamp(writtenRaw, 0, span);

    let status;
    if (span > 0 && written >= span) status = "done";
    else if (written > 0) status = "downloading";
    else status = raw.total_completed === true ? "done" : "pending";

    return {
      key: `conn-${index}`,
      index: index + 1,
      start,
      end,
      downloaded: written,
      status,
    };
  });
}

/**
 * Payload keys that may carry a per-connection / per-range array. The backend
 * currently sends `parts` and `connections` as plain counts, so every key is
 * read defensively: the first non-empty array of objects wins, and anything
 * else (missing, `null`, a number) falls through to the synthesised fallback.
 */
const RANGE_LIST_KEYS = [
  "segment_ranges",
  "connection_ranges",
  "ranges",
  "connections",
  "parts",
];

function pickRangeList(task) {
  for (const key of RANGE_LIST_KEYS) {
    const value = task[key];
    if (Array.isArray(value) && value.length > 0) {
      const looksLikeRanges = value.some(
        (entry) => entry && typeof entry === "object"
      );
      if (looksLikeRanges) return value;
    }
  }
  return null;
}

/** Normalises one entry of a per-connection array; tolerates junk entries. */
function segmentFromRangeEntry(range) {
  const raw = range && typeof range === "object" ? range : {};
  const start = Math.max(
    0,
    Math.floor(toFiniteNumber(raw.start) ?? toFiniteNumber(raw.from) ?? 0)
  );
  const endRaw = toFiniteNumber(raw.end) ?? toFiniteNumber(raw.to);
  const end = Math.max(start, Math.floor(endRaw ?? start));
  const span = end - start;
  const explicit = toFiniteNumber(raw.written) ?? toFiniteNumber(raw.downloaded);
  const completed = raw.completed === true || raw.total_completed === true;

  return {
    start,
    end,
    written: explicit !== null ? clamp(explicit, 0, span) : completed ? span : 0,
    total_completed: completed || (span > 0 && explicit !== null && explicit >= span),
  };
}

/**
 * Connection ranges for the segmented bar and the per-connection table.
 *
 * Prefers real checkpoint ranges (`task.segment_ranges`, or any other
 * per-connection array the backend may start sending) when present; otherwise
 * synthesises `parts` equal byte bands and fills them sequentially from the
 * task's `written` total, so the window still shows the connection regions IDM
 * draws before per-range data is available.
 *
 * @param {{ total_size?: number|null, written?: number, parts?: number|Array<object>|null, connections?: number|Array<object>|null, segment_ranges?: Array<object> }} task
 * @returns {Array<{ start: number, end: number, written: number, total_completed: boolean }>}
 */
export function buildConnectionSegments(task) {
  const t = task && typeof task === "object" ? task : {};

  const list = pickRangeList(t);
  if (list) return list.map(segmentFromRangeEntry);

  const total = toFiniteNumber(t.total_size);
  const parts = Math.floor(toFiniteNumber(t.parts) ?? toFiniteNumber(t.connections) ?? 0);
  const written = Math.max(0, toFiniteNumber(t.written) ?? 0);
  if (total === null || total <= 0 || parts < 1) return [];

  const bandSize = total / parts;
  const segments = [];
  for (let i = 0; i < parts; i += 1) {
    const start = i === 0 ? 0 : Math.round(i * bandSize);
    const end = i === parts - 1 ? total : Math.round((i + 1) * bandSize);
    const span = Math.max(end - start, 0);
    const filled = clamp(written - start, 0, span);
    segments.push({
      start,
      end,
      written: filled,
      total_completed: span > 0 && filled >= span,
    });
  }
  return segments;
}

/**
 * Validates a KB/s limit typed by the user.
 * Returns a positive integer (saturated at u32::MAX), or `null` for junk input.
 *
 * @param {unknown} kbps
 * @returns {number|null}
 */
export function normalizeSpeedLimit(kbps) {
  const n = toFiniteNumber(kbps);
  if (n === null) return null;
  return clamp(Math.round(n), MIN_SPEED_LIMIT_KBPS, MAX_SPEED_LIMIT_KBPS);
}

/**
 * Parses a user-typed speed limit into KB/s.
 * - Empty/zero → unlimited (kbps: null)
 * - Negative → error
 * - Returns KB/s as integer (rounded)
 *
 * @param {string|number|null|undefined} raw
 * @param {"kbps"|"mbps"} unit
 * @returns {{ ok: true, kbps: number|null } | { ok: false, error: 'negative'|'invalid' }}
 */
export function parseSpeedLimitInput(raw, unit = "kbps") {
  if (raw === "" || raw === null || raw === undefined) {
    return { ok: true, kbps: null };
  }
  const n = toFiniteNumber(raw);
  if (n === null) {
    return { ok: false, error: "invalid" };
  }
  if (n < 0) {
    return { ok: false, error: "negative" };
  }
  if (n === 0) {
    return { ok: true, kbps: null }; // unlimited
  }
  const kbps = Math.round(unit === "mbps" ? n * KBPS_PER_MBPS : n);
  return { ok: true, kbps: Math.min(kbps, MAX_SPEED_LIMIT_KBPS) };
}

/**
 * Converts KB/s to a human-readable string for the given locale.
 * null/0 → localized "Unlimited".
 *
 * @param {number|null|undefined} kbps
 * @param {string} lang
 * @returns {string}
 */
export function formatSpeedLimit(kbps, lang = "ar") {
  if (!kbps || kbps <= 0) {
    return resolveTranslation(lang, "downloadWindow.unlimited");
  }
  if (kbps >= KBPS_PER_MBPS && kbps % KBPS_PER_MBPS === 0) {
    return `${kbps / KBPS_PER_MBPS} ${resolveTranslation(lang, "units.mbps")}`;
  }
  return `${kbps} ${resolveTranslation(lang, "units.kbps")}`;
}

/**
 * Determines a sensible display unit and value for prefilling the input.
 * Returns { value: number, unit: "kbps"|"mbps" }.
 *
 * @param {number|null|undefined} kbps
 * @returns {{ value: number, unit: "kbps"|"mbps" }}
 */
export function kbpsToDisplayUnit(kbps) {
  if (!kbps || kbps <= 0) return { value: 0, unit: "kbps" };
  if (kbps >= KBPS_PER_MBPS && kbps % KBPS_PER_MBPS === 0) {
    return { value: kbps / KBPS_PER_MBPS, unit: "mbps" };
  }
  return { value: kbps, unit: "kbps" };
}

/**
 * Whether the task can be resumed later (Range support + known size).
 *
 * Rules:
 * - a known `range_unsupported` error wins: not resumable
 * - an explicit backend `resumable` flag wins when present
 * - otherwise: resumable only when the total size is known
 *
 * @param {{ total_size?: number|null, error_kind?: string|null, resumable?: boolean|null }|null|undefined} task
 * @returns {boolean}
 */
export function isResumable(task) {
  if (!task || typeof task !== "object") return false;
  if (task.error_kind === "range_unsupported") return false;
  if (typeof task.resumable === "boolean") return task.resumable;

  const total = toFiniteNumber(task.total_size);
  return total !== null && total > 0;
}

/**
 * The state that may be displayed. The frontend must never infer completion
 * from a worker exiting, the window closing, or a missing state — only an
 * explicit `Completed` task state is reported as completed.
 *
 * @param {{ state?: string }|null|undefined} task
 * @param {{ windowClosed?: boolean }} [context]
 * @returns {string}
 */
export function deriveDisplayState(task, { windowClosed = false } = {}) {
  // `windowClosed` is intentionally not part of the decision: closing the
  // window says nothing about the download.
  void windowClosed;
  const state = task && typeof task === "object" ? task.state : null;
  return typeof state === "string" && state ? state : "Queued";
}

/**
 * Determines the progress bar fill direction for RTL/LTR layouts.
 * In RTL (Arabic), the progress bar should fill from right to left.
 * In LTR (English), the progress bar should fill from left to right.
 *
 * @param {boolean} isRtl - Whether the current language is RTL
 * @returns {{ direction: 'rtl' | 'ltr', fillStart: 'right' | 'left' }}
 */
export function getProgressBarFillDirection(isRtl) {
  return isRtl
    ? { direction: 'rtl', fillStart: 'right' }
    : { direction: 'ltr', fillStart: 'left' };
}

/**
 * Returns the CSS style object for the progress bar fill element
 * based on the current language direction.
 *
 * @param {boolean} isRtl - Whether the current language is RTL
 * @param {number} percent - Progress percentage (0-100)
 * @returns {object} CSS style properties for the fill element
 */
export function getProgressBarFillStyle(isRtl, percent) {
  const baseStyle = {
    width: `${Math.max(0, Math.min(100, percent))}%`,
    height: '100%',
  };

  if (isRtl) {
    return {
      ...baseStyle,
      marginLeft: 'auto',
      marginRight: 0,
    };
  }

  return {
    ...baseStyle,
    marginLeft: 0,
    marginRight: 'auto',
  };
}

/**
 * Percentage of a download that has been written, as `0..100` with one decimal.
 * Returns `null` when the total size is unknown, so callers can render `—`.
 *
 * @param {number|null|undefined} written
 * @param {number|null|undefined} totalSize
 * @returns {number|null}
 */
export function progressPercent(written, totalSize) {
  return percentOf(written, totalSize);
}

/**
 * Rows for the «حالة التنزيل» tab. Each row carries an i18n `labelKey` for the
 * component to translate, plus an already-formatted `value`, so the same rows
 * are produced in Arabic and English.
 *
 * Order: state, [parts], file size, downloaded (with %), transfer rate,
 * time left, resumable.
 *
 * @param {object|null|undefined} task
 * @param {string} [lang="ar"]
 * @returns {Array<{ key: string, labelKey: string, value: string, valueDir: "ltr"|"auto" }>}
 */
export function buildStatusRows(task, lang = "ar") {
  const row = (key, labelKey, value, valueDir) => ({ key, labelKey, value, valueDir });
  const t = task && typeof task === "object" ? task : {};

  const totalSize = toFiniteNumber(t.total_size);
  const written = Math.max(0, toFiniteNumber(t.written) ?? 0);
  const speed = toFiniteNumber(t.speed_bps);
  const eta = toFiniteNumber(t.eta_secs);
  const parts = toFiniteNumber(t.parts);
  const percent = percentOf(written, totalSize);

  const rows = [
    row("state", "downloadWindow.state", typeof t.state === "string" && t.state ? t.state : "Queued", "auto"),
  ];

  if (parts !== null && parts > 0) {
    rows.push(row("parts", "downloadWindow.parts", String(Math.floor(parts)), "ltr"));
  }

  rows.push(
    row(
      "fileSize",
      "downloadWindow.fileSize",
      totalSize !== null && totalSize > 0
        ? formatBytes(totalSize, lang)
        : resolveTranslation(lang, "task.unknownSize"),
      "ltr"
    ),
    row(
      "downloaded",
      "downloadWindow.downloaded",
      percent === null
        ? formatBytes(written, lang)
        : `${formatBytes(written, lang)} (${formatPercent(percent, lang)})`,
      "ltr"
    ),
    row(
      "transferRate",
      "downloadWindow.speed",
      speed !== null && speed > 0 ? formatSpeed(speed, lang) : "—",
      "ltr"
    ),
    row(
      "timeLeft",
      "downloadWindow.timeLeft",
      eta !== null && eta > 0
        ? formatEta(eta, lang)
        : speed !== null && speed > 0
          ? resolveTranslation(lang, "task.calculating")
          : "—",
      "ltr"
    ),
    row(
      "resumable",
      "downloadWindow.resumable",
      isResumable(t) ? resolveTranslation(lang, "downloadWindow.yes") : resolveTranslation(lang, "downloadWindow.no"),
      "auto"
    )
  );

  return rows;
}

/**
 * Human-readable byte range, e.g. `0 B – 1.2 MB`.
 *
 * @param {number} start
 * @param {number} end
 * @param {string} [lang="ar"]
 * @returns {string}
 */
export function formatRangeInfo(start, end, lang = "ar") {
  const from = Math.max(0, toFiniteNumber(start) ?? 0);
  const to = Math.max(from, toFiniteNumber(end) ?? from);
  if (to === from) return "—";
  return `${formatBytes(from, lang)} – ${formatBytes(to, lang)}`;
}

/**
 * Rows for the per-connection table (N°, downloaded, info/range, status).
 * `status` is one of `done` | `downloading` | `pending`; the component maps it
 * to the `downloadWindow.connDone` / `connDownloading` / `connPending` keys.
 *
 * @param {Array<{start?: number, end?: number, written?: number, total_completed?: boolean}>} segments
 * @param {string} [lang="ar"]
 * @returns {Array<{ key: string, index: number, downloaded: string, info: string, status: string }>}
 */
export function buildConnectionTableRows(segments, lang = "ar") {
  return buildSegmentDetailRows(segments).map((detail) => ({
    key: detail.key,
    index: detail.index,
    downloaded: formatBytes(detail.downloaded, lang),
    info: formatRangeInfo(detail.start, detail.end, lang),
    status: detail.status,
  }));
}

/**
 * Per-connection rows for the live table, derived from whatever the payload
 * currently carries: a real per-range array when one exists, otherwise the
 * synthesised `parts`/`connections` byte bands. Never throws on a missing field.
 *
 * @param {object|null|undefined} task
 * @returns {Array<{ key: string, index: number, start: number, end: number, downloaded: number, status: string }>}
 */
export function normalizeConnectionRanges(task) {
  return buildSegmentDetailRows(buildConnectionSegments(task));
}

/**
 * Keeps each connection's downloaded bytes monotonic across updates of one
 * attempt: `previous` and `next` are keyed by connection (`conn-<n>`), and a
 * value can only grow. Values are clamped to the connection's own span so a
 * re-planned range can never report more bytes than it covers.
 *
 * @param {Array<{ key: string, start: number, end: number, downloaded: number, status: string }>} previous
 * @param {Array<{ key: string, start: number, end: number, downloaded: number, status: string }>} next
 * @returns {Array<{ key: string, index: number, start: number, end: number, downloaded: number, status: string }>}
 */
export function mergeConnectionRowsMonotonic(previous, next) {
  const before = new Map(
    (Array.isArray(previous) ? previous : []).map((row) => [row.key, row])
  );

  return (Array.isArray(next) ? next : []).map((row) => {
    const span = Math.max(row.end - row.start, 0);
    const prior = before.get(row.key);
    const downloaded = prior
      ? Math.max(prior.downloaded, row.downloaded)
      : row.downloaded;
    const bounded = span > 0 ? clamp(downloaded, 0, span) : downloaded;

    let status;
    if (span > 0 && bounded >= span) status = "done";
    else if (bounded > 0) status = "downloading";
    else status = row.status;

    return { ...row, downloaded: bounded, status };
  });
}

/**
 * Stateful tracker for the live connections table. `rows(task, lang)` returns
 * formatted table rows and keeps each connection's downloaded bytes monotonic
 * for the same attempt; a new task id or a restart/retry state transition
 * (see `isProgressResetState`) clears the history so the numbers may restart.
 *
 * @returns {{ rows: (task: object|null|undefined, lang?: string) => Array<{ key: string, index: number, downloaded: string, info: string, status: string }>, reset: () => void }}
 */
export function createConnectionTableTracker() {
  /** @type {string|null} */
  let lastId = null;
  /** @type {string|null} */
  let lastState = null;
  /** @type {Array<object>} */
  let previousRows = [];

  function reset() {
    lastId = null;
    lastState = null;
    previousRows = [];
  }

  return {
    rows(task, lang = "ar") {
      const t = task && typeof task === "object" ? task : {};
      const id = typeof t.id === "string" ? t.id : "";
      const state = typeof t.state === "string" ? t.state : "";
      const isNewAttempt =
        id !== lastId || isProgressResetState(state, lastState);

      if (isNewAttempt) previousRows = [];

      const current = normalizeConnectionRanges(t);
      const merged = mergeConnectionRowsMonotonic(previousRows, current);

      lastId = id;
      lastState = state;
      previousRows = merged;

      return merged.map((row) => ({
        key: row.key,
        index: row.index,
        downloaded: formatBytes(row.downloaded, lang),
        info: formatRangeInfo(row.start, row.end, lang),
        status: row.status,
      }));
    },
    reset,
  };
}

/**
 * Speed-limit tab state. Falls back to an unlimited label when the task has no
 * limit configured.
 *
 * @param {{ speed_limit_kbps?: number|null }} task
 * @param {string} [lang="ar"]
 * @returns {{ enabled: boolean, limitKbps: number|null, label: string }}
 */
export function buildSpeedLimitState(task, lang = "ar") {
  const raw = task && typeof task === "object" ? task.speed_limit_kbps : null;
  const limit = normalizeSpeedLimit(raw);
  const enabled = limit !== null && (raw === 0 || raw === "0" ? false : toFiniteNumber(raw) !== 0);
  return {
    enabled,
    limitKbps: enabled ? limit : null,
    label: enabled
      ? `${limit} ${resolveTranslation(lang, "units.kbps")}`
      : resolveTranslation(lang, "downloadWindow.unlimited"),
  };
}

/**
 * Which action buttons the window should offer for the current state.
 * `cancel` genuinely cancels the task; `hide` only dismisses the window.
 *
 * @param {{ state?: string }|null|undefined} task
 * @param {{ isPending?: boolean }} [context]
 * @returns {{ pause: boolean, resume: boolean, retry: boolean, cancel: boolean, hide: boolean, openFile: boolean, reveal: boolean }}
 */
export function getWindowActionFlags(task, { isPending = false } = {}) {
  const state = task && typeof task === "object" ? task.state : null;
  const completed = state === "Completed";
  const failed = state === "Failed";

  return {
    pause: !isPending && canPauseTask(state),
    resume: !isPending && !failed && canResumeTask(state),
    retry: !isPending && failed,
    cancel: canCancel(state),
    hide: true,
    openFile: completed,
    reveal: completed,
  };
}

/**
 * Details shown by the completion window: file name, size, saved path.
 *
 * @param {object|null|undefined} task
 * @param {string} [lang="ar"]
 * @returns {{ filename: string, size: string, savedPath: string }}
 */
export function buildCompleteDetails(task, lang = "ar") {
  const t = task && typeof task === "object" ? task : {};
  const name = typeof t.filename === "string" && t.filename.trim() ? t.filename.trim() : "—";
  const total = toFiniteNumber(t.total_size);
  const written = toFiniteNumber(t.written);
  const size =
    total !== null && total > 0
      ? formatBytes(total, lang)
      : written !== null && written > 0
        ? formatBytes(written, lang)
        : resolveTranslation(lang, "task.unknownSize");

  const folder =
    (typeof t.folder === "string" && t.folder.trim()) ||
    (typeof t.save_path === "string" && t.save_path.trim()) ||
    "";

  return {
    filename: name,
    size,
    savedPath: folder || "—",
  };
}

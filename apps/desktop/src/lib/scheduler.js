export const STORAGE_KEY = "sayl_scheduler_v1";

export const DEFAULT_SCHEDULE = {
  enabled: true,
  start_hour: 2,
  start_minute: 0,
  stop_hour: 6,
  stop_minute: 0,
  days: [1, 2, 3, 4, 5, 6, 7], // 1 = Mon, 7 = Sun
  queue_size: 2,
  shutdown_when_done: false,
};

export function normalizeSchedule(input) {
  if (!input || typeof input !== "object") {
    return { ...DEFAULT_SCHEDULE };
  }

  const numOr = (val, min, max, fallback) => {
    const n = Number(val);
    return isNaN(n) ? fallback : Math.min(max, Math.max(min, Math.round(n)));
  };

  const days = Array.isArray(input.days)
    ? input.days.map(Number).filter((d) => d >= 1 && d <= 7)
    : DEFAULT_SCHEDULE.days;

  return {
    enabled: typeof input.enabled === "boolean" ? input.enabled : true,
    start_hour: numOr(input.start_hour, 0, 23, DEFAULT_SCHEDULE.start_hour),
    start_minute: numOr(input.start_minute, 0, 59, DEFAULT_SCHEDULE.start_minute),
    stop_hour: numOr(input.stop_hour, 0, 23, DEFAULT_SCHEDULE.stop_hour),
    stop_minute: numOr(input.stop_minute, 0, 59, DEFAULT_SCHEDULE.stop_minute),
    days: Array.from(new Set(days)).sort((a, b) => a - b),
    queue_size: numOr(input.queue_size, 1, 10, DEFAULT_SCHEDULE.queue_size),
    shutdown_when_done: Boolean(input.shutdown_when_done),
  };
}

export function loadSchedule(storage) {
  const store = storage || (typeof localStorage !== "undefined" ? localStorage : null);
  if (!store) return { ...DEFAULT_SCHEDULE };
  try {
    const raw = store.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_SCHEDULE };
    const parsed = JSON.parse(raw);
    return normalizeSchedule(parsed);
  } catch {
    return { ...DEFAULT_SCHEDULE };
  }
}

export function saveSchedule(schedule, storage) {
  const store = storage || (typeof localStorage !== "undefined" ? localStorage : null);
  const normalized = normalizeSchedule(schedule);
  if (store) {
    try {
      store.setItem(STORAGE_KEY, JSON.stringify(normalized));
    } catch {}
  }
  return normalized;
}

export function serializeTime(hour, minute) {
  const h = String(Math.min(23, Math.max(0, Number(hour) || 0))).padStart(2, "0");
  const m = String(Math.min(59, Math.max(0, Number(minute) || 0))).padStart(2, "0");
  return `${h}:${m}`;
}

export function parseTime(timeStr) {
  if (!timeStr || typeof timeStr !== "string") {
    return { hour: 0, minute: 0 };
  }
  const parts = timeStr.split(":");
  const hour = Math.min(23, Math.max(0, parseInt(parts[0], 10) || 0));
  const minute = Math.min(59, Math.max(0, parseInt(parts[1], 10) || 0));
  return { hour, minute };
}

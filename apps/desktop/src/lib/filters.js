/**
 * Pure task filtering module for SAYL download manager.
 * Provides query search, state filter grouping, category matching,
 * and multi-criteria pipeline filtering.
 */

import { classifyFile } from "./classify.js";

export const ACTIVE_STATES = [
  "Queued",
  "Probing",
  "Downloading",
  "Verifying",
  "Finalizing",
];

export const PAUSED_STATES = ["Paused", "Pausing"];

export const COMPLETED_STATES = ["Completed"];

export const FAILED_STATES = ["Failed"];

export const STATE_FILTERS = [
  { id: "all", labelAr: "الكل", labelEn: "All" },
  { id: "active", labelAr: "يعمل", labelEn: "Active" },
  { id: "paused", labelAr: "متوقف", labelEn: "Paused" },
  { id: "completed", labelAr: "مكتمل", labelEn: "Completed" },
  { id: "failed", labelAr: "فشل", labelEn: "Failed" },
];

/**
 * Checks if a task state satisfies the selected state filter.
 * Supports both internal IDs ('all', 'active', 'paused', 'completed', 'failed')
 * and Arabic filter labels ('الكل', 'يعمل', 'متوقف', 'مكتمل', 'فشل').
 *
 * @param {string} taskState
 * @param {string} filter
 * @returns {boolean}
 */
export function matchesStateFilter(taskState, filter = "all") {
  const f = (filter || "all").trim();
  if (f === "all" || f === "الكل") {
    return true;
  }
  if (f === "incomplete" || f === "غير مكتمل") {
    return taskState !== "Completed";
  }
  if (f === "queue" || f === "قائمة الانتظار") {
    return taskState === "Queued";
  }
  if (f === "active" || f === "يعمل") {
    return ACTIVE_STATES.includes(taskState);
  }
  if (f === "paused" || f === "متوقف") {
    return PAUSED_STATES.includes(taskState);
  }
  if (f === "completed" || f === "مكتمل") {
    return COMPLETED_STATES.includes(taskState);
  }
  if (f === "failed" || f === "فشل") {
    return FAILED_STATES.includes(taskState);
  }
  return true;
}

/**
 * Matches a task against a search query string.
 * Query is case-insensitive, trimmed, and tests against filename and URL.
 *
 * @param {{ filename?: string, url?: string }} task
 * @param {string} query
 * @returns {boolean}
 */
export function matchesQuery(task, query = "") {
  const q = (query || "").trim().toLowerCase();
  if (!q) return true;

  const fn = (task?.filename || "").toLowerCase();
  const u = (task?.url || "").toLowerCase();

  return fn.includes(q) || u.includes(q);
}

/**
 * Checks if a task belongs to a specific category.
 *
 * @param {{ filename?: string, mime?: string }} task
 * @param {string} category
 * @returns {boolean}
 */
export function matchesCategory(task, category = "all") {
  const c = (category || "all").trim();
  if (c === "all" || c === "الكل") {
    return true;
  }
  const taskCategory = classifyFile(task?.filename, task?.mime);
  return taskCategory === c;
}

/**
 * Combined filtering helper applying category, state filter, and search query.
 *
 * @param {Array<object>} tasks
 * @param {{ category?: string, state?: string, query?: string }} criteria
 * @returns {Array<object>}
 */
export function filterTasks(tasks, { category = "all", state = "all", query = "" } = {}) {
  if (!Array.isArray(tasks)) return [];

  return tasks.filter((task) => {
    return (
      matchesCategory(task, category) &&
      matchesStateFilter(task.state, state) &&
      matchesQuery(task, query)
    );
  });
}

/**
 * Decides which sidebar filters must change so a just-added task is visible.
 *
 * Capture, Ctrl+V and manual adds all land here: a new download must never be
 * added silently behind a filter that hides it. When the current filters would
 * hide the task, they are widened to "الكل"/"All" and the search is cleared —
 * the widest possible view. The caller is then responsible for selecting and
 * scrolling to the row.
 *
 * @param {{ filename?: string, url?: string, state?: string }} task
 * @param {{ category?: string, state?: string, query?: string }} [filters]
 * @returns {{ visible: boolean, category: string, state: string, query: string }}
 *   `visible` is true when the task passes the *current* filters; otherwise
 *   `category`/`state`/`query` hold the widened values to apply.
 */
export function resolveNewTaskVisibility(task, filters = {}) {
  const category = filters.category || "all";
  const state = filters.state || "all";
  const query = filters.query || "";

  const visible =
    matchesCategory(task, category) &&
    matchesStateFilter(task?.state, state) &&
    matchesQuery(task, query);

  if (visible) return { visible: true, category, state, query };

  // "الكل" (all) is the widest view; reset the free-text search too, which hides
  // rows by substring match that the new task may not satisfy.
  return { visible: false, category: "all", state: "all", query: "" };
}

/**
 * Computes state counts for the state filter row badges.
 *
 * @param {Array<{ state?: string }>} tasks
 * @returns {{ all: number, active: number, paused: number, completed: number, failed: number }}
 */
export function getStateCounts(tasks) {
  const counts = {
    all: 0,
    active: 0,
    paused: 0,
    completed: 0,
    failed: 0,
    incomplete: 0,
    queue: 0,
  };

  if (!Array.isArray(tasks)) return counts;

  counts.all = tasks.length;
  for (const t of tasks) {
    if (t.state !== "Completed") {
      counts.incomplete += 1;
    }
    if (t.state === "Queued") {
      counts.queue += 1;
    }
    if (ACTIVE_STATES.includes(t.state)) {
      counts.active += 1;
    } else if (PAUSED_STATES.includes(t.state)) {
      counts.paused += 1;
    } else if (COMPLETED_STATES.includes(t.state)) {
      counts.completed += 1;
    } else if (FAILED_STATES.includes(t.state)) {
      counts.failed += 1;
    }
  }

  return counts;
}

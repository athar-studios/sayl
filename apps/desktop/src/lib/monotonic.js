/**
 * Monotonic merge helpers for SAYL task progress.
 *
 * Ensures downloaded bytes and segment progress never decrease for the same task
 * unless the task state explicitly indicates a restart or cancellation.
 * This prevents UI flicker when out-of-order events arrive (e.g., Progress event
 * with higher bytes followed by Checkpoint/StateChanged with lower bytes).
 */

/**
 * Returns true if a state transition represents a legitimate reset of progress.
 * Only these states allow progress to decrease.
 */
export function isProgressResetState(newState, oldState) {
  // Going to Queued/Probing from a non-pending state means restart
  if ((newState === "Queued" || newState === "Probing") &&
      oldState !== "Queued" && oldState !== "Probing" && oldState !== "Pending") {
    return true;
  }
  // Cancelled resets everything
  if (newState === "Cancelled") return true;
  // Failed could be retry which restarts
  if (newState === "Failed" && oldState !== "Failed") return true;
  return false;
}

/**
 * Merges an incoming task update into the existing task, ensuring
 * written bytes, speed, eta, and segment progress never decrease
 * unless the state indicates a legitimate restart.
 *
 * @param {object} existing - Current task in store
 * @param {object} incoming - New task data from event/upsert
 * @returns {object} Merged task
 */
export function mergeTaskMonotonic(existing, incoming) {
  if (!existing) return incoming;
  if (!incoming) return existing;

  const newState = incoming.state;
  const oldState = existing.state;

  // If state indicates a progress reset, allow full replacement
  if (isProgressResetState(newState, oldState)) {
    return { ...existing, ...incoming };
  }

  // Otherwise, merge monotonically
  const merged = { ...existing };

  // written: never decrease
  if (typeof incoming.written === "number" && typeof existing.written === "number") {
    merged.written = Math.max(existing.written, incoming.written);
  } else if (typeof incoming.written === "number") {
    merged.written = incoming.written;
  }

  // total_size: prefer incoming if valid, but don't shrink if existing has data
  if (typeof incoming.total_size === "number" && incoming.total_size > 0) {
    if (typeof existing.total_size === "number" && existing.total_size > 0) {
      // Keep the larger total_size if both valid (server may correct)
      merged.total_size = Math.max(existing.total_size, incoming.total_size);
    } else {
      merged.total_size = incoming.total_size;
    }
  }

  // speed_bps: use incoming (latest measurement)
  if (typeof incoming.speed_bps === "number") {
    merged.speed_bps = incoming.speed_bps;
  }

  // eta_secs: use incoming
  if (typeof incoming.eta_secs === "number") {
    merged.eta_secs = incoming.eta_secs;
  }

  // state: always use incoming (state transitions are authoritative)
  if (typeof incoming.state === "string") {
    merged.state = incoming.state;
  }

  // error, error_kind: use incoming
  if (incoming.error !== undefined) merged.error = incoming.error;
  if (incoming.error_kind !== undefined) merged.error_kind = incoming.error_kind;

  // completed_at: use incoming if set
  if (incoming.completed_at) merged.completed_at = incoming.completed_at;

  // resumable: use incoming
  if (typeof incoming.resumable === "boolean") merged.resumable = incoming.resumable;

  // mime, filename, url, save_path: use incoming if present
  if (incoming.mime) merged.mime = incoming.mime;
  if (incoming.filename) merged.filename = incoming.filename;
  if (incoming.url) merged.url = incoming.url;
  if (incoming.save_path) merged.save_path = incoming.save_path;

  // segment_ranges: for parallel downloads, merge by taking union of ranges
  // but only if not a reset state
  if (Array.isArray(incoming.segment_ranges) && Array.isArray(existing.segment_ranges)) {
    // Keep the union of ranges (max end per segment index)
    // For simplicity, if incoming has more completed ranges, use it
    const incCompleted = incoming.segment_ranges.filter(r => r.completed).length;
    const existCompleted = existing.segment_ranges.filter(r => r.completed).length;
    if (incCompleted >= existCompleted) {
      merged.segment_ranges = incoming.segment_ranges;
    }
  } else if (Array.isArray(incoming.segment_ranges)) {
    merged.segment_ranges = incoming.segment_ranges;
  }

  return merged;
}

/**
 * Merges an array of incoming tasks into existing tasks array monotonically.
 */
export function mergeTasksMonotonic(existingTasks, incomingTasks) {
  if (!Array.isArray(incomingTasks)) return existingTasks;
  if (!Array.isArray(existingTasks)) return incomingTasks;

  const existingMap = new Map(existingTasks.map(t => [t.id, t]));
  const result = [];

  for (const incoming of incomingTasks) {
    const existing = existingMap.get(incoming.id);
    if (existing) {
      result.push(mergeTaskMonotonic(existing, incoming));
      existingMap.delete(incoming.id);
    } else {
      // New task
      result.push(incoming);
    }
  }

  // Add remaining existing tasks that weren't in incoming
  for (const remaining of existingMap.values()) {
    result.push(remaining);
  }

  return result;
}
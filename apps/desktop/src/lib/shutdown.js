/**
 * Pure helper module for SAYL machine shutdown countdown and safety evaluations.
 */

export const BLOCKED_REASONS = {
  failed_download: {
    code: "failed_download",
    shortAr: "فشل",
    shortEn: "Failed",
    messageAr: "تعذر إيقاف التشغيل: توجد تنزيلات فاشلة",
    messageEn: "Shutdown refused: failed downloads exist",
  },
  paused_download: {
    code: "paused_download",
    shortAr: "متوقف",
    shortEn: "Paused",
    messageAr: "تعذر إيقاف التشغيل: توجد تنزيلات متوقفة مؤقتاً",
    messageEn: "Shutdown refused: paused downloads exist",
  },
  active_download: {
    code: "active_download",
    shortAr: "قيد التنزيل",
    shortEn: "Active",
    messageAr: "تعذر إيقاف التشغيل: توجد تنزيلات قيد التنزيل",
    messageEn: "Shutdown refused: downloads are still in progress",
  },
  no_tasks: {
    code: "no_tasks",
    shortAr: "لا توجد تنزيلات",
    shortEn: "No tasks",
    messageAr: "تعذر إيقاف التشغيل: لا توجد تنزيلات",
    messageEn: "Shutdown refused: no download tasks found",
  },
};

/**
 * Pure helper deciding whether all downloads have finished.
 * Rules:
 * - Empty task list -> false
 * - Any non-terminal task (Downloading, Queued, Probing, Verifying, Finalizing,
 *   Pausing, Paused, Failed, Cancelling) -> false
 * - Only Completed or Cancelled tasks -> true
 *
 * @param {Array<{ state: string }>} tasks
 * @returns {boolean}
 */
export function areAllDownloadsFinished(tasks) {
  if (!Array.isArray(tasks) || tasks.length === 0) {
    return false;
  }

  return tasks.every((t) => t && (t.state === "Completed" || t.state === "Cancelled"));
}

/**
 * Returns a user-facing explanation why machine shutdown was refused.
 *
 * @param {string|null|undefined} reason Code from backend blocked_reason
 * @param {string} lang Language code ('ar' or 'en')
 * @returns {string}
 */
export function getBlockedReasonMessage(reason, lang = "ar") {
  if (!reason || !BLOCKED_REASONS[reason]) {
    return lang === "ar"
      ? "تعذر إيقاف تشغيل الجهاز"
      : "Machine shutdown was refused";
  }

  const item = BLOCKED_REASONS[reason];
  return lang === "ar" ? item.messageAr : item.messageEn;
}

/**
 * Returns a short label for the blocked reason.
 *
 * @param {string|null|undefined} reason
 * @param {string} lang
 * @returns {string}
 */
export function getBlockedReasonShort(reason, lang = "ar") {
  if (!reason || !BLOCKED_REASONS[reason]) {
    return "";
  }
  const item = BLOCKED_REASONS[reason];
  return lang === "ar" ? item.shortAr : item.shortEn;
}

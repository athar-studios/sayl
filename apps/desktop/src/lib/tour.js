/**
 * First-run tour state and geometry helpers.
 * Pure logic module for 6-step walkthrough.
 */

export const TOUR_STORAGE_KEY = "sayl.tour.completed.v1";

export const TOUR_STEPS = [
  {
    id: "add_download",
    selector: ".add-btn, #tour-btn-add",
    fallbackSelector: ".toolbar-main-actions button",
    titleKey: "tour.step1Title",
    descKey: "tour.step1Desc",
  },
  {
    id: "categories_sidebar",
    selector: ".app-sidebar, #tour-sidebar",
    fallbackSelector: ".sidebar-categories",
    titleKey: "tour.step2Title",
    descKey: "tour.step2Desc",
  },
  {
    id: "search_filter",
    selector: "#tour-search-filter, .filter-controls-bar",
    fallbackSelector: ".search-and-view-controls",
    titleKey: "tour.step3Title",
    descKey: "tour.step3Desc",
  },
  {
    id: "task_progress",
    selector: "#tour-task-area, .task-list-container, .app-main",
    fallbackSelector: ".app-content",
    titleKey: "tour.step4Title",
    descKey: "tour.step4Desc",
  },
  {
    id: "settings_button",
    selector: "button[title*='settings' i], button[title*='إعدادات' i], .toolbar-main-actions button:last-child",
    fallbackSelector: ".toolbar-main-actions",
    titleKey: "tour.step5Title",
    descKey: "tour.step5Desc",
  },
  {
    id: "browser_extension",
    selector: "#tour-extension-section, #tour-nav-extension, .modal-card",
    fallbackSelector: ".app-layout",
    titleKey: "tour.step6Title",
    descKey: "tour.step6Desc",
    hasGuideAction: true,
  },
];

export function isTourCompleted(storage = typeof localStorage !== "undefined" ? localStorage : null) {
  if (!storage) return false;
  try {
    return storage.getItem(TOUR_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

export function setTourCompleted(storage = typeof localStorage !== "undefined" ? localStorage : null) {
  if (!storage) return;
  try {
    storage.setItem(TOUR_STORAGE_KEY, "1");
  } catch {}
}

export function clearTourCompleted(storage = typeof localStorage !== "undefined" ? localStorage : null) {
  if (!storage) return;
  try {
    storage.removeItem(TOUR_STORAGE_KEY);
  } catch {}
}

export function getNextStepIndex(currentIndex, total = TOUR_STEPS.length) {
  if (typeof currentIndex !== "number" || isNaN(currentIndex) || currentIndex < 0) {
    return 0;
  }
  if (currentIndex < total - 1) {
    return currentIndex + 1;
  }
  return total - 1;
}

export function getPrevStepIndex(currentIndex) {
  if (typeof currentIndex !== "number" || isNaN(currentIndex) || currentIndex <= 0) {
    return 0;
  }
  return currentIndex - 1;
}

/**
 * Maps arrow keys to navigation actions based on RTL / LTR layout.
 * In RTL: ArrowLeft advances forward (Next), ArrowRight goes backward (Previous).
 * In LTR: ArrowRight advances forward (Next), ArrowLeft goes backward (Previous).
 * Escape triggers skip.
 */
export function getRtlKeyAction(key, isRtl = false) {
  if (key === "Escape") return "skip";
  if (isRtl) {
    if (key === "ArrowLeft") return "next";
    if (key === "ArrowRight") return "prev";
  } else {
    if (key === "ArrowRight") return "next";
    if (key === "ArrowLeft") return "prev";
  }
  return null;
}

/**
 * Computes cutout rectangle coordinates or falls back to centered coordinates
 * when target element is missing or collapsed.
 */
export function computeCutoutBounds(
  selector,
  getElementFn = (s) => (typeof document !== "undefined" ? document.querySelector(s) : null),
  windowSize = { width: 1024, height: 768 }
) {
  const el = typeof getElementFn === "function" ? getElementFn(selector) : null;
  if (!el || typeof el.getBoundingClientRect !== "function") {
    return {
      top: Math.round((windowSize?.height || 768) / 2),
      left: Math.round((windowSize?.width || 1024) / 2),
      width: 0,
      height: 0,
      isFallback: true,
    };
  }

  const rect = el.getBoundingClientRect();
  if (!rect || rect.width <= 0 || rect.height <= 0) {
    return {
      top: Math.round((windowSize?.height || 768) / 2),
      left: Math.round((windowSize?.width || 1024) / 2),
      width: 0,
      height: 0,
      isFallback: true,
    };
  }

  const padding = 6;
  return {
    top: Math.max(0, rect.top - padding),
    left: Math.max(0, rect.left - padding),
    width: rect.width + padding * 2,
    height: rect.height + padding * 2,
    isFallback: false,
  };
}

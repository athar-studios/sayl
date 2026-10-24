import { writable } from "svelte/store";
import { DEFAULT_SETTINGS, normalizeSettings } from "./settings.js";

export const toasts = writable([]);

export function showToast(message, type = "error") {
  const id = Date.now() + Math.random().toString(36).substring(2, 7);
  toasts.update((current) => [...current, { id, message, type }]);
  setTimeout(() => {
    toasts.update((current) => current.filter((t) => t.id !== id));
  }, 4500);
}

export function removeToast(id) {
  toasts.update((current) => current.filter((t) => t.id !== id));
}

let tauriCore = null;

async function getTauriCore() {
  if (typeof window === "undefined") return null;
  if (tauriCore) return tauriCore;
  if (window.__TAURI_INTERNALS__ || window.__TAURI__) {
    try {
      tauriCore = await import("@tauri-apps/api/core");
      return tauriCore;
    } catch {
      return null;
    }
  }
  return null;
}

/**
 * Robust wrapper around Tauri `invoke` that catches any boundary error
 * and surfaces it in the toast notification bar.
 */
export async function safeInvoke(command, args = {}) {
  try {
    const core = await getTauriCore();
    if (core && typeof core.invoke === "function") {
      return await core.invoke(command, args);
    }
    // Safe mock fallback for headless/browser preview
    return handleBrowserMock(command, args);
  } catch (err) {
    const msg = typeof err === "string" ? err : err?.message || String(err);
    showToast(msg, "error");
    throw err;
  }
}

/**
 * Safe mock state for local browser development/preview.
 */
const mockTasks = [
  {
    id: "01900000-0000-7000-8000-000000000001",
    url: "https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso",
    filename: "ubuntu-24.04-desktop-amd64.iso",
    state: "Downloading",
    total_size: 6100000000,
    written: 3200000000,
    speed_bps: 14500000,
    eta_secs: 200,
    error: null,
    mime: "application/x-iso9660-image",
    parts: 8,
  },
  {
    id: "01900000-0000-7000-8000-000000000002",
    url: "https://example.com/archive.zip",
    filename: "archive.zip",
    state: "Completed",
    total_size: 250000000,
    written: 250000000,
    speed_bps: 0,
    eta_secs: null,
    error: null,
    mime: "application/zip",
    parts: 4,
  },
];

let mockSettings = {
  ...DEFAULT_SETTINGS,
  download_dir: "C:\\Users\\User\\Downloads",
};

let mockShutdownStatus = {
  armed: false,
  seconds_left: null,
  blocked_reason: null,
};

function normalizeUrl(url) {
  if (!url) return "";
  let clean = url.trim();
  while (clean.endsWith("/")) {
    clean = clean.slice(0, -1);
  }
  return clean.toLowerCase();
}

function handleBrowserMock(command, args) {
  switch (command) {
    case "list_tasks":
      return [...mockTasks];

    case "add_task": {
      const cleanUrl = (args.url || "").trim();
      if (!cleanUrl) {
        throw new Error("URL cannot be empty");
      }

      if (args.force !== true) {
        const norm = normalizeUrl(cleanUrl);
        const existing = mockTasks.find(
          (t) => normalizeUrl(t.url) === norm && t.state !== "Cancelled"
        );
        if (existing) {
          return { kind: "duplicate", existing: { ...existing } };
        }
      }

      const rawName = cleanUrl.split("/").pop()?.split(/[?#]/)[0] || "download.bin";
      const newTask = {
        id: "mock-" + Date.now(),
        url: cleanUrl,
        filename: rawName,
        state: "Downloading",
        total_size: 104857600,
        written: 0,
        speed_bps: 1048576,
        eta_secs: 100,
        error: null,
        mime: null,
        parts: 4,
      };
      mockTasks.unshift(newTask);
      return { kind: "added", task: newTask };
    }

    case "pause_task": {
      const task = mockTasks.find((t) => t.id === args.id);
      if (task) {
        task.state = "Paused";
        task.speed_bps = 0;
        task.eta_secs = null;
      }
      return null;
    }

    case "start_task": {
      const task = mockTasks.find((t) => t.id === args.id);
      if (task) {
        task.state = "Downloading";
        task.speed_bps = 2097152;
      }
      return null;
    }

    case "cancel_task": {
      const task = mockTasks.find((t) => t.id === args.id);
      if (task) {
        task.state = "Cancelled";
        task.speed_bps = 0;
        task.eta_secs = null;
      }
      return null;
    }

    case "get_settings":
      return { ...mockSettings };

    case "set_settings": {
      mockSettings = normalizeSettings(args.settings);
      return { ...mockSettings };
    }

    case "open_download_folder":
    case "reveal_task":
    case "open_task_file":
      return null;

    case "arm_shutdown": {
      mockShutdownStatus = {
        armed: true,
        seconds_left: args.seconds || 60,
        blocked_reason: null,
      };
      return { ...mockShutdownStatus };
    }

    case "cancel_shutdown": {
      mockShutdownStatus = {
        armed: false,
        seconds_left: null,
        blocked_reason: null,
      };
      return { ...mockShutdownStatus };
    }

    case "get_shutdown_status":
      return { ...mockShutdownStatus };

    default:
      return null;
  }
}

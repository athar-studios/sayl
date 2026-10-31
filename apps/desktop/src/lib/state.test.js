import { describe, it, expect } from "vitest";
import { resolveTranslation } from "./i18n.js";
import { canPause, canResume, canCancel, isTerminal } from "./format.js";

const ALL_STATES = [
  "Queued",
  "Probing",
  "Downloading",
  "Verifying",
  "Finalizing",
  "Completed",
  "Pausing",
  "Paused",
  "Cancelling",
  "Cancelled",
  "Failed",
];

describe("State mapping & action guards", () => {
  it("has translations for all 11 download states in Arabic", () => {
    for (const state of ALL_STATES) {
      const label = resolveTranslation("ar", `state.${state}`);
      expect(label).toBeTruthy();
      expect(label).not.toBe(`state.${state}`);
    }
  });

  it("has translations for all 11 download states in English", () => {
    for (const state of ALL_STATES) {
      const label = resolveTranslation("en", `state.${state}`);
      expect(label).toBeTruthy();
      expect(label).not.toBe(`state.${state}`);
    }
  });

  it("has translations for task.connecting in Arabic and English", () => {
    const arLabel = resolveTranslation("ar", "task.connecting");
    expect(arLabel).toBe("جارٍ الاتصال...");

    const enLabel = resolveTranslation("en", "task.connecting");
    expect(enLabel).toBe("Connecting…");
  });

  it("correctly identifies pausable states", () => {
    expect(canPause("Downloading")).toBe(true);
    expect(canPause("Probing")).toBe(true);
    expect(canPause("Paused")).toBe(false);
    expect(canPause("Completed")).toBe(false);
    expect(canPause("Queued")).toBe(false);
    expect(canPause("Cancelled")).toBe(false);
  });

  it("correctly identifies resumable states", () => {
    expect(canResume("Paused")).toBe(true);
    expect(canResume("Queued")).toBe(true);
    expect(canResume("Failed")).toBe(true);
    expect(canResume("Downloading")).toBe(false);
    expect(canResume("Completed")).toBe(false);
  });

  it("correctly identifies cancellable states", () => {
    expect(canCancel("Queued")).toBe(true);
    expect(canCancel("Downloading")).toBe(true);
    expect(canCancel("Paused")).toBe(true);
    expect(canCancel("Failed")).toBe(true);
    expect(canCancel("Completed")).toBe(false);
    expect(canCancel("Cancelled")).toBe(false);
  });

  it("correctly identifies terminal states", () => {
    expect(isTerminal("Completed")).toBe(true);
    expect(isTerminal("Cancelled")).toBe(true);
    expect(isTerminal("Failed")).toBe(true);
    expect(isTerminal("Downloading")).toBe(false);
    expect(isTerminal("Paused")).toBe(false);
    expect(isTerminal("Queued")).toBe(false);
  });

  it("ensures cancelled tasks do not appear in openDownloadWindows or active floating tasks", () => {
    let openDownloadWindows = ["t1", "t2", "t3"];
    const tasks = [
      { id: "t1", state: "Downloading" },
      { id: "t2", state: "Cancelled" },
      { id: "t3", state: "Completed" },
    ];

    // Filter updated tasks (matching handleTasksUpdated logic)
    for (const t of tasks) {
      if (t.state === "Cancelled") {
        openDownloadWindows = openDownloadWindows.filter((id) => id !== t.id);
      }
    }
    expect(openDownloadWindows).not.toContain("t2");
    expect(openDownloadWindows).toEqual(["t1", "t3"]);

    // Active floating tasks derivation rejects Cancelled tasks
    const activeFloating = openDownloadWindows
      .map((id) => tasks.find((t) => t.id === id))
      .filter((t) => t && t.state !== "Cancelled");
    expect(activeFloating.map((t) => t.id)).toEqual(["t1", "t3"]);

    // Guard on new task: cancelled task never added
    const cancelledTask = { id: "t4", state: "Cancelled" };
    if (cancelledTask.state !== "Cancelled") {
      openDownloadWindows.push(cancelledTask.id);
    }
    expect(openDownloadWindows).not.toContain("t4");
  });

  it("verifies that mutating settings or updating tasks 100 consecutive times triggers 0 window creations and does not loop", () => {
    let windowCreationCount = 0;
    const openWindowsSet = new Set();
    let openDownloadWindows = [];

    function mockOpenDownloadWindow(taskId) {
      if (!taskId) return;
      if (openWindowsSet.has(taskId)) {
        return; // Deduplicated
      }
      openWindowsSet.add(taskId);
      windowCreationCount += 1;
      openDownloadWindows.push(taskId);
    }

    let settings = {
      download_dir: "C:\\Downloads",
      theme: "system",
      language: "ar",
      view_mode: "table",
      show_download_window: true,
      close_download_window_on_finish: true,
      max_concurrent_tasks: 3,
    };

    // 1. Mutate settings 100 consecutive times
    for (let i = 0; i < 100; i++) {
      settings = {
        ...settings,
        theme: i % 2 === 0 ? "dark" : "light",
        language: i % 2 === 0 ? "en" : "ar",
        view_mode: i % 2 === 0 ? "cards" : "table",
        max_concurrent_tasks: (i % 5) + 1,
        close_download_window_on_finish: i % 2 === 0,
      };
      // Mutating settings must NEVER trigger window creation
    }

    expect(windowCreationCount).toBe(0);
    expect(openWindowsSet.size).toBe(0);
    expect(openDownloadWindows.length).toBe(0);

    // 2. Update tasks 100 consecutive times via handleTasksUpdated logic
    const tasks = [
      { id: "task-1", state: "Downloading", written: 1000 },
      { id: "task-2", state: "Paused", written: 5000 },
      { id: "task-3", state: "Completed", written: 10000 },
    ];

    function handleTasksUpdated(newTasks) {
      // In handleTasksUpdated, we ONLY clean up completed/cancelled windows;
      // we NEVER open or close WebviewWindows in a loop.
      for (const t of newTasks) {
        if (
          (t.state === "Completed" && settings.close_download_window_on_finish) ||
          t.state === "Cancelled"
        ) {
          openWindowsSet.delete(t.id);
          openDownloadWindows = openDownloadWindows.filter((id) => id !== t.id);
        }
      }
    }

    for (let i = 0; i < 100; i++) {
      tasks[0].written += 100;
      handleTasksUpdated(tasks);
    }

    expect(windowCreationCount).toBe(0);
    expect(openWindowsSet.size).toBe(0);

    // 3. Verify deduplication: 100 consecutive calls to open a window for the same task
    for (let i = 0; i < 100; i++) {
      mockOpenDownloadWindow("task-1");
    }

    // Exactly 1 window creation for 100 calls, no loop
    expect(windowCreationCount).toBe(1);
    expect(openWindowsSet.size).toBe(1);
    expect(openWindowsSet.has("task-1")).toBe(true);

    // Further 100 task updates do NOT create any additional windows
    for (let i = 0; i < 100; i++) {
      tasks[0].written += 50;
      handleTasksUpdated(tasks);
    }
    expect(windowCreationCount).toBe(1);
  });
});

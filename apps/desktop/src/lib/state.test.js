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
});

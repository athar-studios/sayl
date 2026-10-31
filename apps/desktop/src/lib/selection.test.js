import { describe, it, expect } from "vitest";
import {
  handleItemClick,
  selectAll,
  clearSelection,
  toggleSelect,
  moveSelection,
} from "./selection.js";

describe("selection utilities", () => {
  const items = [
    { id: "task-1", filename: "file1.zip" },
    { id: "task-2", filename: "file2.iso" },
    { id: "task-3", filename: "file3.tar" },
    { id: "task-4", filename: "file4.mp4" },
    { id: "task-5", filename: "file5.exe" },
  ];

  it("selects a single item on plain click", () => {
    const res = handleItemClick({
      items,
      selectedIds: [],
      clickedId: "task-2",
      event: {},
    });
    expect(res.selectedIds).toEqual(["task-2"]);
    expect(res.lastAnchorId).toBe("task-2");
  });

  it("replaces previous selection on plain click", () => {
    const res = handleItemClick({
      items,
      selectedIds: ["task-1", "task-3"],
      clickedId: "task-4",
      event: {},
      lastAnchorId: "task-1",
    });
    expect(res.selectedIds).toEqual(["task-4"]);
    expect(res.lastAnchorId).toBe("task-4");
  });

  it("toggles item selection on Ctrl/Cmd click", () => {
    // Add to selection
    const res1 = handleItemClick({
      items,
      selectedIds: ["task-1"],
      clickedId: "task-3",
      event: { ctrlKey: true },
      lastAnchorId: "task-1",
    });
    expect(res1.selectedIds).toEqual(["task-1", "task-3"]);
    expect(res1.lastAnchorId).toBe("task-3");

    // Remove from selection
    const res2 = handleItemClick({
      items,
      selectedIds: ["task-1", "task-3"],
      clickedId: "task-1",
      event: { metaKey: true },
      lastAnchorId: "task-3",
    });
    expect(res2.selectedIds).toEqual(["task-3"]);
  });

  it("selects range between anchor and target on Shift click", () => {
    const res = handleItemClick({
      items,
      selectedIds: ["task-2"],
      clickedId: "task-4",
      event: { shiftKey: true },
      lastAnchorId: "task-2",
    });
    expect(res.selectedIds).toEqual(["task-2", "task-3", "task-4"]);
    expect(res.lastAnchorId).toBe("task-2");
  });

  it("selects reverse range if target is above anchor on Shift click", () => {
    const res = handleItemClick({
      items,
      selectedIds: ["task-4"],
      clickedId: "task-2",
      event: { shiftKey: true },
      lastAnchorId: "task-4",
    });
    expect(res.selectedIds).toEqual(["task-2", "task-3", "task-4"]);
    expect(res.lastAnchorId).toBe("task-4");
  });

  it("unions range with existing selection when Shift + Ctrl are held", () => {
    const res = handleItemClick({
      items,
      selectedIds: ["task-1"],
      clickedId: "task-5",
      event: { shiftKey: true, ctrlKey: true },
      lastAnchorId: "task-3",
    });
    expect(res.selectedIds).toEqual(["task-1", "task-3", "task-4", "task-5"]);
  });

  it("falls back to single select if anchor is invalid on Shift click", () => {
    const res = handleItemClick({
      items,
      selectedIds: [],
      clickedId: "task-3",
      event: { shiftKey: true },
      lastAnchorId: "unknown-anchor",
    });
    expect(res.selectedIds).toEqual(["task-3"]);
    expect(res.lastAnchorId).toBe("task-3");
  });

  it("handles non-existent clicked item safely", () => {
    const res = handleItemClick({
      items,
      selectedIds: ["task-1"],
      clickedId: "not-found",
      event: {},
    });
    expect(res.selectedIds).toEqual(["task-1"]);
  });

  it("selectAll returns all item ids", () => {
    expect(selectAll(items)).toEqual([
      "task-1",
      "task-2",
      "task-3",
      "task-4",
      "task-5",
    ]);
  });

  it("clearSelection returns an empty array", () => {
    expect(clearSelection()).toEqual([]);
  });

  it("toggleSelect toggles correctly", () => {
    expect(toggleSelect(["task-1"], "task-2")).toEqual(["task-1", "task-2"]);
    expect(toggleSelect(["task-1", "task-2"], "task-1")).toEqual(["task-2"]);
  });

  it("moveSelection navigates down and up", () => {
    // Navigate down from task-1
    const down = moveSelection({
      items,
      selectedIds: ["task-1"],
      direction: "down",
    });
    expect(down.selectedIds).toEqual(["task-2"]);
    expect(down.activeId).toBe("task-2");

    // Navigate up from task-2
    const up = moveSelection({
      items,
      selectedIds: ["task-2"],
      direction: "up",
    });
    expect(up.selectedIds).toEqual(["task-1"]);
    expect(up.activeId).toBe("task-1");
  });

  it("moveSelection extends selection with Shift", () => {
    const extended = moveSelection({
      items,
      selectedIds: ["task-2"],
      direction: "down",
      shiftKey: true,
      lastAnchorId: "task-2",
    });
    expect(extended.selectedIds).toEqual(["task-2", "task-3"]);
  });
});

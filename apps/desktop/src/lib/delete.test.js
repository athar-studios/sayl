import { describe, it, expect, beforeEach } from "vitest";
import { safeInvoke, mockTasks } from "./tauri.js";
import ar from "../i18n/ar.js";
import en from "../i18n/en.js";

describe("delete vs cancel and delete dialog logic", () => {
  beforeEach(() => {
    // Reset mock tasks for testing
    mockTasks.length = 0;
    mockTasks.push(
      {
        id: "task-1",
        filename: "ubuntu.iso",
        state: "Downloading",
        speed_bps: 1000,
        eta_secs: 10,
      },
      {
        id: "task-2",
        filename: "nodejs.msi",
        state: "Completed",
        speed_bps: 0,
        eta_secs: null,
      },
      {
        id: "task-3",
        filename: "video.mp4",
        state: "Paused",
        speed_bps: 0,
        eta_secs: null,
      }
    );
  });

  describe("mock IPC invoke contract", () => {
    it("cancel_task stops the task, marks state as Cancelled, but retains it in task list", async () => {
      await safeInvoke("cancel_task", { id: "task-1" });

      const found = mockTasks.find((t) => t.id === "task-1");
      expect(found).toBeDefined();
      expect(found.state).toBe("Cancelled");
      expect(found.speed_bps).toBe(0);
      expect(found.eta_secs).toBeNull();
      // Length unchanged
      expect(mockTasks.length).toBe(3);
    });

    it("delete_tasks removes single or multiple tasks permanently from task list", async () => {
      const removed = await safeInvoke("delete_tasks", {
        ids: ["task-1", "task-3"],
        delete_files: true,
      });

      expect(removed).toEqual(["task-1", "task-3"]);
      expect(mockTasks.length).toBe(1);
      expect(mockTasks[0].id).toBe("task-2");
      expect(mockTasks.find((t) => t.id === "task-1")).toBeUndefined();
      expect(mockTasks.find((t) => t.id === "task-3")).toBeUndefined();
    });

    it("delete_task command removes single task permanently", async () => {
      await safeInvoke("delete_task", { id: "task-2", delete_files: false });
      expect(mockTasks.length).toBe(2);
      expect(mockTasks.find((t) => t.id === "task-2")).toBeUndefined();
    });
  });

  describe("delete dialog options and i18n strings", () => {
    it("provides all required delete dialog keys in Arabic", () => {
      expect(ar.deleteDialog).toBeDefined();
      expect(ar.deleteDialog.title).toBe("حذف التنزيل");
      expect(ar.deleteDialog.titleMultiple).toBe("حذف التنزيلات المحددة");
      expect(ar.deleteDialog.alsoDeleteFile).toBe("حذف الملف من القرص أيضاً");
      expect(ar.deleteDialog.removeOnly).toBe("إزالة من القائمة فقط");
      expect(ar.deleteDialog.deleteFileAndRemove).toBe("حذف الملف وإزالته من القائمة");
      expect(ar.deleteDialog.cancel).toBe("إلغاء");
      expect(ar.table.cancel).toBe("إلغاء");
    });

    it("provides all required delete dialog keys in English", () => {
      expect(en.deleteDialog).toBeDefined();
      expect(en.deleteDialog.title).toBe("Delete Download");
      expect(en.deleteDialog.titleMultiple).toBe("Delete Selected Downloads");
      expect(en.deleteDialog.alsoDeleteFile).toBe("Also delete file from disk");
      expect(en.deleteDialog.removeOnly).toBe("Remove from list only");
      expect(en.deleteDialog.deleteFileAndRemove).toBe("Delete file and remove from list");
      expect(en.deleteDialog.cancel).toBe("Cancel");
      expect(en.table.cancel).toBe("Cancel");
    });

    it("computes correct button label based on deleteFiles checkbox state", () => {
      const getButtonLabel = (deleteFiles, lang = "ar") => {
        const dict = lang === "ar" ? ar : en;
        return deleteFiles
          ? dict.deleteDialog.deleteFileAndRemove
          : dict.deleteDialog.removeOnly;
      };

      // Unchecked: "إزالة من القائمة فقط"
      expect(getButtonLabel(false, "ar")).toBe("إزالة من القائمة فقط");
      expect(getButtonLabel(false, "en")).toBe("Remove from list only");

      // Checked: "إزالة وحذف الملف"
      expect(getButtonLabel(true, "ar")).toBe("حذف الملف وإزالته من القائمة");
      expect(getButtonLabel(true, "en")).toBe("Delete file and remove from list");
    });

    it("delete-with-file prompts say the file is deleted from disk (ar+en)", () => {
      for (const dict of [ar, en]) {
        expect(dict.deleteDialog.promptSingleWithFile).toContain("{filename}");
        expect(dict.deleteDialog.promptMultipleWithFile).toContain("{count}");
      }
      expect(ar.deleteDialog.promptSingleWithFile).toContain("القرص");
      expect(ar.deleteDialog.promptMultipleWithFile).toContain("القرص");
      expect(en.deleteDialog.promptSingleWithFile).toContain("from disk");
      expect(en.deleteDialog.promptMultipleWithFile).toContain("from disk");
      expect(ar.contextMenu.deleteWithFile).toBe("حذف مع الملف...");
      expect(en.contextMenu.deleteWithFile).toBe("Delete with file...");
    });

    it("formats single vs multi-task prompt correctly", () => {
      const formatPrompt = (count, filename, lang = "ar") => {
        const dict = lang === "ar" ? ar : en;
        if (count > 1) {
          return dict.deleteDialog.promptMultiple.replace("{count}", String(count));
        }
        return dict.deleteDialog.promptSingle.replace("{filename}", filename);
      };

      expect(formatPrompt(1, "test.zip", "ar")).toBe(
        "هل تريد بالتأكيد إزالة 'test.zip' من القائمة؟"
      );
      expect(formatPrompt(3, "", "ar")).toBe(
        "هل تريد بالتأكيد إزالة 3 تنزيلات من القائمة؟"
      );
      expect(formatPrompt(1, "test.zip", "en")).toBe(
        "Are you sure you want to remove 'test.zip' from the list?"
      );
      expect(formatPrompt(5, "", "en")).toBe(
        "Are you sure you want to remove 5 downloads from the list?"
      );
    });
  });

  describe("multi-selection deletion targeting logic", () => {
    function resolveIdsToDelete({ targetId, selectedIds = [], fallbackId = null }) {
      if (typeof targetId === "string" && targetId) {
        if (selectedIds.includes(targetId)) {
          return [...selectedIds];
        }
        return [targetId];
      }
      if (selectedIds.length > 0) {
        return [...selectedIds];
      }
      if (fallbackId) {
        return [fallbackId];
      }
      return [];
    }

    it("deletes all selected items when target is among selected items", () => {
      const ids = resolveIdsToDelete({
        targetId: "task-2",
        selectedIds: ["task-1", "task-2", "task-3"],
      });
      expect(ids).toEqual(["task-1", "task-2", "task-3"]);
    });

    it("deletes only target item when target is not among selected items", () => {
      const ids = resolveIdsToDelete({
        targetId: "task-4",
        selectedIds: ["task-1", "task-2"],
      });
      expect(ids).toEqual(["task-4"]);
    });

    it("deletes selected items when triggered from toolbar without targetId", () => {
      const ids = resolveIdsToDelete({
        targetId: null,
        selectedIds: ["task-1", "task-3"],
      });
      expect(ids).toEqual(["task-1", "task-3"]);
    });

    it("falls back to fallback single selected ID when selectedIds is empty", () => {
      const ids = resolveIdsToDelete({
        targetId: null,
        selectedIds: [],
        fallbackId: "task-2",
      });
      expect(ids).toEqual(["task-2"]);
    });

    it("returns empty array when nothing is selected", () => {
      const ids = resolveIdsToDelete({
        targetId: null,
        selectedIds: [],
        fallbackId: null,
      });
      expect(ids).toEqual([]);
    });
  });
});

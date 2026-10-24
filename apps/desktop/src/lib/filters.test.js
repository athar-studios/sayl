import { describe, it, expect } from "vitest";
import {
  ACTIVE_STATES,
  PAUSED_STATES,
  COMPLETED_STATES,
  FAILED_STATES,
  STATE_FILTERS,
  matchesStateFilter,
  matchesQuery,
  matchesCategory,
  filterTasks,
  getStateCounts,
} from "./filters.js";

describe("Task Filtering Logic (filters.js)", () => {
  describe("State filter mapping", () => {
    it("exports all 5 state filter definitions", () => {
      expect(STATE_FILTERS.map((f) => f.id)).toEqual([
        "all",
        "active",
        "paused",
        "completed",
        "failed",
      ]);
    });

    it("matches 'all' (الكل) for any state", () => {
      const sampleStates = ["Queued", "Downloading", "Paused", "Completed", "Failed", "Cancelled"];
      for (const st of sampleStates) {
        expect(matchesStateFilter(st, "all")).toBe(true);
        expect(matchesStateFilter(st, "الكل")).toBe(true);
      }
    });

    it("matches 'active' (يعمل) for Queued, Probing, Downloading, Verifying, Finalizing", () => {
      for (const st of ACTIVE_STATES) {
        expect(matchesStateFilter(st, "active")).toBe(true);
        expect(matchesStateFilter(st, "يعمل")).toBe(true);
      }
      expect(matchesStateFilter("Paused", "active")).toBe(false);
      expect(matchesStateFilter("Completed", "active")).toBe(false);
      expect(matchesStateFilter("Failed", "active")).toBe(false);
      expect(matchesStateFilter("Cancelled", "active")).toBe(false);
    });

    it("matches 'paused' (متوقف) for Paused and Pausing", () => {
      for (const st of PAUSED_STATES) {
        expect(matchesStateFilter(st, "paused")).toBe(true);
        expect(matchesStateFilter(st, "متوقف")).toBe(true);
      }
      expect(matchesStateFilter("Downloading", "paused")).toBe(false);
      expect(matchesStateFilter("Completed", "paused")).toBe(false);
      expect(matchesStateFilter("Failed", "paused")).toBe(false);
    });

    it("matches 'completed' (مكتمل) for Completed only", () => {
      for (const st of COMPLETED_STATES) {
        expect(matchesStateFilter(st, "completed")).toBe(true);
        expect(matchesStateFilter(st, "مكتمل")).toBe(true);
      }
      expect(matchesStateFilter("Downloading", "completed")).toBe(false);
      expect(matchesStateFilter("Paused", "completed")).toBe(false);
      expect(matchesStateFilter("Failed", "completed")).toBe(false);
    });

    it("matches 'failed' (فشل) for Failed only", () => {
      for (const st of FAILED_STATES) {
        expect(matchesStateFilter(st, "failed")).toBe(true);
        expect(matchesStateFilter(st, "فشل")).toBe(true);
      }
      expect(matchesStateFilter("Downloading", "failed")).toBe(false);
      expect(matchesStateFilter("Completed", "failed")).toBe(false);
      expect(matchesStateFilter("Paused", "failed")).toBe(false);
    });
  });

  describe("Query matching against filename and URL", () => {
    const task = {
      filename: "Ubuntu-24.04-desktop-amd64.iso",
      url: "https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso",
    };

    it("matches filename case-insensitively and trimmed", () => {
      expect(matchesQuery(task, "ubuntu")).toBe(true);
      expect(matchesQuery(task, "  UBUNTU  ")).toBe(true);
      expect(matchesQuery(task, "desktop")).toBe(true);
      expect(matchesQuery(task, "iso")).toBe(true);
      expect(matchesQuery(task, "fedora")).toBe(false);
    });

    it("matches URL case-insensitively and trimmed", () => {
      expect(matchesQuery(task, "releases.ubuntu.com")).toBe(true);
      expect(matchesQuery(task, "https://releases")).toBe(true);
      expect(matchesQuery(task, "24.04")).toBe(true);
      expect(matchesQuery(task, "nonexistent-domain.com")).toBe(false);
    });

    it("empty query matches every task", () => {
      expect(matchesQuery(task, "")).toBe(true);
      expect(matchesQuery(task, "   ")).toBe(true);
      expect(matchesQuery(task, null)).toBe(true);
    });
  });

  describe("Combined category + state + query filtering", () => {
    const taskList = [
      { id: "1", filename: "video_one.mp4", url: "https://example.com/v1.mp4", state: "Downloading" },
      { id: "2", filename: "video_two.mkv", url: "https://mirror.org/v2.mkv", state: "Completed" },
      { id: "3", filename: "audio_track.mp3", url: "https://example.com/music.mp3", state: "Downloading" },
      { id: "4", filename: "document.pdf", url: "https://docs.site/manual.pdf", state: "Paused" },
      { id: "5", filename: "installer.exe", url: "https://apps.net/setup.exe", state: "Failed" },
      { id: "6", filename: "archive.zip", url: "https://data.com/archive.zip", state: "Completed" },
    ];

    it("filters by category alone", () => {
      const videos = filterTasks(taskList, { category: "video" });
      expect(videos.map((t) => t.id)).toEqual(["1", "2"]);

      const docs = filterTasks(taskList, { category: "documents" });
      expect(docs.map((t) => t.id)).toEqual(["4"]);
    });

    it("filters by state alone", () => {
      const active = filterTasks(taskList, { state: "active" });
      expect(active.map((t) => t.id)).toEqual(["1", "3"]);

      const completed = filterTasks(taskList, { state: "completed" });
      expect(completed.map((t) => t.id)).toEqual(["2", "6"]);

      const failed = filterTasks(taskList, { state: "failed" });
      expect(failed.map((t) => t.id)).toEqual(["5"]);
    });

    it("filters by query alone", () => {
      const matched = filterTasks(taskList, { query: "example.com" });
      expect(matched.map((t) => t.id)).toEqual(["1", "3"]);
    });

    it("combines category + state + query simultaneously", () => {
      // Category: video, State: active, Query: "video_one"
      const res1 = filterTasks(taskList, {
        category: "video",
        state: "active",
        query: "video_one",
      });
      expect(res1.map((t) => t.id)).toEqual(["1"]);

      // Category: video, State: completed, Query: "mirror"
      const res2 = filterTasks(taskList, {
        category: "video",
        state: "completed",
        query: "mirror",
      });
      expect(res2.map((t) => t.id)).toEqual(["2"]);

      // Category: video, State: active, Query: "video_two" (which is completed, so should return empty)
      const res3 = filterTasks(taskList, {
        category: "video",
        state: "active",
        query: "video_two",
      });
      expect(res3).toEqual([]);
    });

    it("accurately computes state counts", () => {
      const counts = getStateCounts(taskList);
      expect(counts.all).toBe(6);
      expect(counts.active).toBe(2); // "1", "3"
      expect(counts.paused).toBe(1); // "4"
      expect(counts.completed).toBe(2); // "2", "6"
      expect(counts.failed).toBe(1); // "5"
    });
  });
});

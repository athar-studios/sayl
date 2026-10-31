import { describe, it, expect } from "vitest";
import {
  buildPrepareCaptureTaskArgs,
  defaultSavePathForCapture,
  resolveCaptureSavePath,
  isCaptureOfferPayload,
  enqueueCaptureOffer,
  dequeueCaptureOffer,
} from "./capture.js";
import { resolveNewTaskVisibility } from "./filters.js";
import { extractUrls, splitClipboardIntoActions } from "./clipboardPaste.js";

describe("capture.js prepare_capture_task args", () => {
  const settings = { download_dir: "C:\\Users\\User\\Downloads" };

  it("buildPrepareCaptureTaskArgs matches Tauri camelCase savePath", () => {
    const args = buildPrepareCaptureTaskArgs({
      id: "01900000-0000-7000-8000-000000000099",
      filename: "setup.exe",
      folder: "D:\\Custom\\Folder",
      settings,
    });
    expect(args).toEqual({
      id: "01900000-0000-7000-8000-000000000099",
      filename: "setup.exe",
      savePath: "D:\\Custom\\Folder",
    });
  });

  it("defaults savePath to category subfolder under download_dir when folder empty", () => {
    const args = buildPrepareCaptureTaskArgs({
      id: "task-1",
      filename: "movie.mkv",
      category: "video",
      settings,
    });
    expect(args.savePath).toBe("C:\\Users\\User\\Downloads\\Video");
    expect(args).toHaveProperty("savePath");
    expect(args).not.toHaveProperty("save_path");
  });

  it("resolveCaptureSavePath prefers dialog folder", () => {
    expect(resolveCaptureSavePath(settings, "E:\\Save", "programs", "a.exe")).toBe("E:\\Save");
  });

  it("defaultSavePathForCapture classifies by filename when category omitted", () => {
    expect(defaultSavePathForCapture("C:/Downloads", undefined, "doc.pdf")).toBe(
      "C:/Downloads/Documents"
    );
  });
});

describe("capture-offer queue (one browser capture → one dialog)", () => {
  const makeOffer = (task_id, filename = "file.zip") => ({
    task_id,
    filename,
    url: `https://example.com/${filename}`,
    size: 1024,
  });

  it("enqueues a valid offer and preserves order", () => {
    let q = [];
    q = enqueueCaptureOffer(q, makeOffer("t1"));
    q = enqueueCaptureOffer(q, makeOffer("t2"));
    q = enqueueCaptureOffer(q, makeOffer("t3"));
    expect(q.map((o) => o.task_id)).toEqual(["t1", "t2", "t3"]);
  });

  it("drops invalid payloads silently", () => {
    expect(isCaptureOfferPayload(null)).toBe(false);
    expect(isCaptureOfferPayload({ task_id: "" })).toBe(false);
    expect(isCaptureOfferPayload({ task_id: "  " })).toBe(false);
    expect(isCaptureOfferPayload({ task_id: 42 })).toBe(false);
    expect(isCaptureOfferPayload(makeOffer("t1"))).toBe(true);

    let q = enqueueCaptureOffer([], { task_id: "" }); // empty id
    q = enqueueCaptureOffer(q, null);
    q = enqueueCaptureOffer(q, { foo: "bar" });
    q = enqueueCaptureOffer(q, makeOffer("t1"));
    expect(q.map((o) => o.task_id)).toEqual(["t1"]);
  });

  it("deduplicates by task_id within the queue", () => {
    let q = enqueueCaptureOffer([], makeOffer("t1"));
    q = enqueueCaptureOffer(q, makeOffer("t1"));
    q = enqueueCaptureOffer(q, makeOffer("t2"));
    expect(q.map((o) => o.task_id)).toEqual(["t1", "t2"]);
  });

  it("ignores a duplicate of the currently displayed offer", () => {
    let q = enqueueCaptureOffer([], makeOffer("t1"));
    const showing = (o) => o.task_id === "t1"; // mimics isCaptureOfferShowing
    q = enqueueCaptureOffer(q, makeOffer("t1"), showing);
    q = enqueueCaptureOffer(q, makeOffer("t2"), showing);
    expect(q.map((o) => o.task_id)).toEqual(["t1", "t2"]);
  });

  it("dequeueCaptureOffer returns the head and the rest", () => {
    let q = [makeOffer("a"), makeOffer("b"), makeOffer("c")];
    let { next, rest } = dequeueCaptureOffer(q);
    expect(next.task_id).toBe("a");
    expect(rest.map((o) => o.task_id)).toEqual(["b", "c"]);

    ({ next, rest } = dequeueCaptureOffer(rest));
    expect(next.task_id).toBe("b");
    expect(rest.map((o) => o.task_id)).toEqual(["c"]);

    ({ next, rest } = dequeueCaptureOffer(rest));
    expect(next.task_id).toBe("c");
    expect(rest).toEqual([]);

    // empty queue
    ({ next, rest } = dequeueCaptureOffer([]));
    expect(next).toBeNull();
    expect(rest).toEqual([]);
  });

  it("simulates App.svelte confirm/cancel flow: a second capture waits while the first dialog is open", () => {
    function makeUi() {
      const ui = { queue: [], shown: null, open: false };
      const showNext = () => {
        if (ui.open && ui.shown) return;
        const { next, rest } = dequeueCaptureOffer(ui.queue);
        if (!next) {
          ui.open = false;
          return;
        }
        ui.queue = rest;
        ui.shown = next;
        ui.open = true;
      };
      return {
        receive(payload) {
          ui.queue = enqueueCaptureOffer(ui.queue, payload, (o) =>
            ui.open && o.task_id === ui.shown?.task_id
          );
          showNext();
        },
        dismiss() {
          const shown = ui.shown;
          ui.open = false;
          ui.shown = null;
          showNext();
          return shown;
        },
        get shown() {
          return ui.shown;
        },
        get open() {
          return ui.open;
        },
        get queue() {
          return ui.queue;
        },
      };
    }

    // t1 arrives → shown
    const ui = makeUi();
    ui.receive(makeOffer("t1"));
    expect(ui.shown.task_id).toBe("t1");
    expect(ui.open).toBe(true);

    // t2 arrives while dialog open → queued
    ui.receive(makeOffer("t2"));
    expect(ui.shown.task_id).toBe("t1");
    expect(ui.queue.map((o) => o.task_id)).toEqual(["t2"]);

    // user confirms first → t2 becomes shown
    const first = ui.dismiss();
    expect(first.task_id).toBe("t1");
    expect(ui.shown.task_id).toBe("t2");
    expect(ui.open).toBe(true);

    // user cancels second → nothing left
    const second = ui.dismiss();
    expect(second.task_id).toBe("t2");
    expect(ui.open).toBe(false);
  });
});

describe("new-task visibility (filters switch to 'الكل' when needed)", () => {
  it("returns visible=true when current filters already show the task", () => {
    const task = { filename: "movie.mp4", url: "https://example.com/movie.mp4", state: "Downloading" };
    const filters = { category: "video", state: "active", query: "" };
    const res = resolveNewTaskVisibility(task, filters);
    expect(res.visible).toBe(true);
    expect(res.category).toBe("video");
    expect(res.state).toBe("active");
    expect(res.query).toBe("");
  });

  it("switches category to 'all' when the selected category would hide the task", () => {
    const task = { filename: "doc.pdf", url: "https://example.com/doc.pdf", state: "Queued" };
    const filters = { category: "video", state: "all", query: "" };
    const res = resolveNewTaskVisibility(task, filters);
    expect(res.visible).toBe(false);
    expect(res.category).toBe("all");
    expect(res.state).toBe("all");
    expect(res.query).toBe("");
  });

  it("switches state filter to 'all' when it would hide the task", () => {
    const task = { filename: "setup.exe", url: "https://example.com/setup.exe", state: "Queued" };
    const filters = { category: "all", state: "completed", query: "" };
    const res = resolveNewTaskVisibility(task, filters);
    expect(res.visible).toBe(false);
    expect(res.category).toBe("all");
    expect(res.state).toBe("all");
    expect(res.query).toBe("");
  });

  it("clears a search query that would hide the task", () => {
    const task = { filename: "archive.zip", url: "https://example.com/archive.zip", state: "Downloading" };
    const filters = { category: "all", state: "all", query: "not-matching" };
    const res = resolveNewTaskVisibility(task, filters);
    expect(res.visible).toBe(false);
    expect(res.category).toBe("all");
    expect(res.state).toBe("all");
    expect(res.query).toBe("");
  });

  it("works with undefined filters (defaults to all)", () => {
    const task = { filename: "test.txt", url: "https://example.com/test.txt", state: "Queued" };
    const res = resolveNewTaskVisibility(task, {});
    expect(res.visible).toBe(true);
    expect(res.category).toBe("all");
    expect(res.state).toBe("all");
    expect(res.query).toBe("");
  });
});

describe("clipboard URL extraction", () => {
  it("extracts a single https URL", () => {
    expect(extractUrls("https://example.com/file.zip")).toEqual([
      "https://example.com/file.zip",
    ]);
  });

  it("extracts multiple URLs separated by newlines or spaces", () => {
    const text = [
      "https://example.com/a.zip",
      "http://example.com/b.iso",
      "ftp://ftp.example.com/c.bin",
    ].join("\n");
    expect(extractUrls(text)).toEqual([
      "https://example.com/a.zip",
      "http://example.com/b.iso",
      "ftp://ftp.example.com/c.bin",
    ]);
  });

  it("drops invalid schemes and de-duplicates", () => {
    const text = [
      "javascript:alert(1)",
      "https://example.com/keep.zip",
      "https://example.com/keep.zip",
      "file:///C:/Windows/file.iso",
    ].join(" ");
    expect(extractUrls(text)).toEqual(["https://example.com/keep.zip"]);
  });

  it("splitClipboardIntoActions routes single URL to 'single', multiple to 'batch'", () => {
    expect(splitClipboardIntoActions(["https://example.com/only.zip"])).toEqual({
      kind: "single",
      url: "https://example.com/only.zip",
    });
    expect(splitClipboardIntoActions([
      "https://example.com/a.zip",
      "https://example.com/b.zip",
    ])).toEqual({
      kind: "batch",
      urls: [
        "https://example.com/a.zip",
        "https://example.com/b.zip",
      ],
    });
  });
});

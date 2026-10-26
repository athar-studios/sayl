import { describe, it, expect } from "vitest";
import {
  buildPrepareCaptureTaskArgs,
  defaultSavePathForCapture,
  resolveCaptureSavePath,
} from "./capture.js";

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

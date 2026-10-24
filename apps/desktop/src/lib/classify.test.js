import { describe, it, expect } from "vitest";
import {
  CATEGORIES,
  CATEGORY_IDS,
  classifyFile,
  getCategoryLabel,
  getCategoryCounts,
} from "./classify.js";

describe("File classification (classify.js)", () => {
  it("exports category list covering all 7 categories", () => {
    expect(CATEGORIES.map((c) => c.id)).toEqual([
      "all",
      "video",
      "audio",
      "documents",
      "programs",
      "compressed",
      "other",
    ]);
    expect(CATEGORY_IDS).toContain("all");
    expect(CATEGORY_IDS).toContain("video");
    expect(CATEGORY_IDS).toContain("audio");
    expect(CATEGORY_IDS).toContain("documents");
    expect(CATEGORY_IDS).toContain("programs");
    expect(CATEGORY_IDS).toContain("compressed");
    expect(CATEGORY_IDS).toContain("other");
  });

  it("classifies every category's common extensions correctly", () => {
    // Video extensions
    const videoSamples = ["clip.mp4", "movie.mkv", "film.avi", "vid.mov", "stream.webm", "tv.flv", "show.wmv", "cam.m4v"];
    for (const file of videoSamples) {
      expect(classifyFile(file)).toBe("video");
    }

    // Audio extensions
    const audioSamples = ["track.mp3", "sound.wav", "song.flac", "podcast.aac", "audio.ogg", "voice.m4a", "record.opus"];
    for (const file of audioSamples) {
      expect(classifyFile(file)).toBe("audio");
    }

    // Documents extensions
    const docSamples = ["paper.pdf", "doc.docx", "old.doc", "sheet.xlsx", "book.xls", "slides.pptx", "notes.txt", "readme.md", "data.csv"];
    for (const file of docSamples) {
      expect(classifyFile(file)).toBe("documents");
    }

    // Programs extensions
    const programSamples = ["setup.exe", "installer.msi", "app.apk", "tool.AppImage", "package.deb", "dist.rpm", "binary.pkg", "server.jar"];
    for (const file of programSamples) {
      expect(classifyFile(file)).toBe("programs");
    }

    // Compressed extensions
    const archiveSamples = ["backup.zip", "data.rar", "archive.7z", "source.tar", "dist.gz", "pack.bz2", "system.iso", "os.dmg"];
    for (const file of archiveSamples) {
      expect(classifyFile(file)).toBe("compressed");
    }
  });

  it("handles uppercase extensions correctly", () => {
    expect(classifyFile("VIDEO.MP4")).toBe("video");
    expect(classifyFile("AUDIO.MP3")).toBe("audio");
    expect(classifyFile("DOCUMENT.PDF")).toBe("documents");
    expect(classifyFile("INSTALLER.EXE")).toBe("programs");
    expect(classifyFile("ARCHIVE.ZIP")).toBe("compressed");
    expect(classifyFile("SYSTEM.ISO")).toBe("compressed");
  });

  it("falls back to MIME when the extension is unknown", () => {
    expect(classifyFile("mystery.xyz", "video/mp4")).toBe("video");
    expect(classifyFile("download.dat", "audio/mpeg")).toBe("audio");
    expect(classifyFile("stream.bin", "video/webm; codecs=vp9")).toBe("video");
    expect(classifyFile("file.custom", "application/pdf")).toBe("documents");
    expect(classifyFile("raw.data", "application/zip")).toBe("compressed");
    expect(classifyFile("run.bin", "application/x-msdownload")).toBe("programs");
  });

  it("falls back to MIME when the extension is missing entirely", () => {
    expect(classifyFile("blob", "video/webm")).toBe("video");
    expect(classifyFile("track", "audio/flac")).toBe("audio");
    expect(classifyFile("docfile", "application/pdf")).toBe("documents");
    expect(classifyFile("setup", "application/x-msdownload")).toBe("programs");
    expect(classifyFile("archive", "application/zip")).toBe("compressed");
  });

  it("maps unknown extension + unknown MIME to أخرى (other)", () => {
    const category = classifyFile("mystery.xyz", "application/x-unknown-data");
    expect(category).toBe("other");
    expect(getCategoryLabel(category, "ar")).toBe("أخرى");
    expect(getCategoryLabel(category, "en")).toBe("Other");

    // Also when both filename and mime are undefined / empty
    expect(classifyFile("")).toBe("other");
    expect(classifyFile(null, null)).toBe("other");
  });

  it("accurately calculates per-category counts across tasks", () => {
    const tasks = [
      { filename: "clip.mp4" },
      { filename: "music.mp3" },
      { filename: "notes.pdf" },
      { filename: "setup.exe" },
      { filename: "archive.zip" },
      { filename: "mystery.bin", mime: "video/webm" },
      { filename: "unknown.xyz", mime: "application/unknown" },
    ];

    const counts = getCategoryCounts(tasks);
    expect(counts.all).toBe(7);
    expect(counts.video).toBe(2); // clip.mp4 + mystery.bin with video/webm
    expect(counts.audio).toBe(1);
    expect(counts.documents).toBe(1);
    expect(counts.programs).toBe(1);
    expect(counts.compressed).toBe(1);
    expect(counts.other).toBe(1);
  });
});

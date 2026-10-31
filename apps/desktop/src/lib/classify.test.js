import { describe, it, expect } from "vitest";
import {
  CATEGORIES,
  CATEGORY_IDS,
  classifyFile,
  getCategoryLabel,
  getCategoryCounts,
  splitStemExtension,
  nextNumberedFilename,
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

describe("Auto-rename on collision (IDM style)", () => {
  it("splits the stem from the last extension only", () => {
    expect(splitStemExtension("Zed-x86_64.exe")).toEqual({
      stem: "Zed-x86_64",
      extension: ".exe",
    });
    expect(splitStemExtension("archive.tar.gz")).toEqual({
      stem: "archive.tar",
      extension: ".gz",
    });
    expect(splitStemExtension("myfile")).toEqual({ stem: "myfile", extension: "" });
    // A leading dot is part of the name; a trailing dot leaves an empty extension.
    expect(splitStemExtension(".gitignore")).toEqual({ stem: ".gitignore", extension: "" });
    expect(splitStemExtension("trailing.")).toEqual({ stem: "trailing", extension: "." });
    expect(splitStemExtension(null)).toEqual({ stem: "", extension: "" });
  });

  it("numbers a plain file name", () => {
    expect(nextNumberedFilename("Zed-x86_64.exe")).toBe("Zed-x86_64 (1).exe");
    expect(nextNumberedFilename("report.pdf")).toBe("report (1).pdf");
  });

  it("increments an existing number instead of nesting it", () => {
    expect(nextNumberedFilename("Zed-x86_64 (1).exe")).toBe("Zed-x86_64 (2).exe");
    expect(nextNumberedFilename("Zed-x86_64 (9).exe")).toBe("Zed-x86_64 (10).exe");
  });

  it("keeps the other numbers inside a stem untouched", () => {
    expect(nextNumberedFilename("file1_v2.0_test3.exe")).toBe("file1_v2.0_test3 (1).exe");
    expect(nextNumberedFilename("file1_v2.0_test3 (1).exe")).toBe("file1_v2.0_test3 (2).exe");
    expect(nextNumberedFilename("file1_v2.0_test3 (2).exe")).toBe("file1_v2.0_test3 (3).exe");
  });

  it("handles names without an extension", () => {
    expect(nextNumberedFilename("myfile")).toBe("myfile (1)");
    expect(nextNumberedFilename("myfile (1)")).toBe("myfile (2)");
    // README.md collides with a folder/file named README.md -> README (1).md
    expect(nextNumberedFilename("README.md")).toBe("README (1).md");
  });

  it("handles names with several dots, numbering the last extension", () => {
    expect(nextNumberedFilename("archive.tar.gz")).toBe("archive.tar (1).gz");
    expect(nextNumberedFilename("archive.tar (1).gz")).toBe("archive.tar (2).gz");
    expect(nextNumberedFilename("my.backup.file.tar.bz2")).toBe("my.backup.file.tar (1).bz2");
  });

  it("walks the sequential (1), (2), (3) increments", () => {
    let name = "setup.exe";
    const produced = [];
    for (let i = 0; i < 3; i += 1) {
      name = nextNumberedFilename(name);
      produced.push(name);
    }
    expect(produced).toEqual(["setup (1).exe", "setup (2).exe", "setup (3).exe"]);
  });

  it("leaves a name that only looks numbered below (1) alone", () => {
    // "(0)" and "(01)" are not counters, so they stay part of the stem.
    expect(nextNumberedFilename("draft (0).txt")).toBe("draft (0) (1).txt");
    expect(nextNumberedFilename("draft (01).txt")).toBe("draft (01) (1).txt");
  });
});

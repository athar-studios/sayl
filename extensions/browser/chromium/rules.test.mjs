import { test, describe } from "node:test";
import assert from "node:assert/strict";
import {
  DEFAULT_EXTENSIONS,
  DEFAULT_MIN_SIZE_MB,
  parseExtensionList,
  extractBasename,
  extractExtension,
  matchesExtension,
  passesSize,
  isAltBypassed,
  sanitizeUrlForLog,
  shouldInterceptDownload
} from "./rules.mjs";

describe("rules.mjs pure logic", () => {
  test("extractBasename handles various paths and URLs", () => {
    assert.equal(extractBasename("https://example.com/downloads/archive.tar.gz?token=abc"), "archive.tar.gz");
    assert.equal(extractBasename("C:\\Users\\User\\Downloads\\setup.exe"), "setup.exe");
    assert.equal(extractBasename("/home/user/downloads/video.mp4#t=10"), "video.mp4");
    assert.equal(extractBasename("https://example.com/path%20with%20spaces/file%20with%20spaces.zip"), "file with spaces.zip");
    assert.equal(extractBasename(""), "");
  });

  test("extractExtension correctly isolates extension", () => {
    assert.equal(extractExtension("archive.zip"), "zip");
    assert.equal(extractExtension("setup.EXE"), "exe");
    assert.equal(extractExtension("archive.tar.gz"), "gz");
    assert.equal(extractExtension(".gitignore"), "");
    assert.equal(extractExtension("no_extension"), "");
  });

  test("matchesExtension matches allowed extensions case-insensitively", () => {
    assert.equal(matchesExtension("test.ZIP"), true);
    assert.equal(matchesExtension("document.PDF"), true);
    assert.equal(matchesExtension("movie.mkv"), true);
    assert.equal(matchesExtension("100MB.dat"), true);
    assert.equal(matchesExtension("page.html"), false);
    assert.equal(matchesExtension("image.png"), false);
    assert.equal(matchesExtension("script.js"), false);

    // Custom list
    assert.equal(matchesExtension("image.png", "png, jpg, gif"), true);
    assert.equal(matchesExtension("document.pdf", ["png", "jpg"]), false);
  });

  test("passesSize behaves as required", () => {
    const oneMb = 1024 * 1024;
    // Unknown size (-1 or 0) passes the size check
    assert.equal(passesSize(-1, 1), true, "-1 unknown size must pass");
    assert.equal(passesSize(0, 1), true, "0 unknown size must pass");
    assert.equal(passesSize(null, 1), true, "null size must pass");
    assert.equal(passesSize(undefined, 1), true, "undefined size must pass");

    // Exact or above threshold
    assert.equal(passesSize(oneMb, 1), true, "1 MB exactly must pass 1 MB threshold");
    assert.equal(passesSize(2 * oneMb, 1), true, "2 MB must pass 1 MB threshold");

    // Below threshold
    assert.equal(passesSize(500 * 1024, 1), false, "500 KB must fail 1 MB threshold");
    assert.equal(passesSize(100, 1), false, "100 bytes must fail 1 MB threshold");
  });

  test("isAltBypassed honors TTL and URL matching", () => {
    const now = 100000;
    const records = {
      "https://example.com/file.zip": now - 3000, // 3s ago (within 10s TTL)
      "https://example.com/old.zip": now - 15000  // 15s ago (expired)
    };

    assert.equal(isAltBypassed("https://example.com/file.zip", records, now), true);
    assert.equal(isAltBypassed("https://example.com/file.zip?query=1", records, now), true);
    assert.equal(isAltBypassed("https://example.com/old.zip", records, now), false);
    assert.equal(isAltBypassed("https://example.com/other.zip", records, now), false);
  });

  test("sanitizeUrlForLog strips queries and cookies, keeping origin + basename", () => {
    assert.equal(
      sanitizeUrlForLog("https://cdn.example.com/path/to/archive.zip?auth=secretToken&id=1"),
      "https://cdn.example.com/archive.zip"
    );
    assert.equal(
      sanitizeUrlForLog("http://files.site.org/downloads/setup.msi#section"),
      "http://files.site.org/setup.msi"
    );
    assert.equal(sanitizeUrlForLog("https://site.org/"), "https://site.org");
  });

  describe("shouldInterceptDownload decisions", () => {
    const baseSettings = {
      enabled: true,
      minSizeMb: 1,
      extensions: DEFAULT_EXTENSIONS
    };

    test("1. allowed extension/size is intercepted", () => {
      const item = {
        id: 1,
        url: "https://example.com/bigfile.iso",
        filename: "bigfile.iso",
        totalBytes: 50 * 1024 * 1024 // 50 MB
      };
      const res = shouldInterceptDownload({ item, settings: baseSettings });
      assert.equal(res.intercept, true);
      assert.equal(res.reason, "allowed");
      assert.equal(res.filename, "bigfile.iso");
      assert.equal(res.size, 50 * 1024 * 1024);
    });

    test("2. too small is skipped", () => {
      const item = {
        id: 2,
        url: "https://example.com/tiny.pdf",
        filename: "tiny.pdf",
        totalBytes: 500 * 1024 // 500 KB < 1 MB
      };
      const res = shouldInterceptDownload({ item, settings: baseSettings });
      assert.equal(res.intercept, false);
      assert.equal(res.reason, "too_small");
    });

    test("3. unknown size (-1 or 0) passes", () => {
      const itemZero = {
        id: 3,
        url: "https://example.com/stream.mp4",
        filename: "stream.mp4",
        totalBytes: 0,
        fileSize: 0
      };
      const resZero = shouldInterceptDownload({ item: itemZero, settings: baseSettings });
      assert.equal(resZero.intercept, true);
      assert.equal(resZero.reason, "allowed");

      const itemNegative = {
        id: 4,
        url: "https://example.com/archive.zip",
        filename: "archive.zip",
        totalBytes: -1,
        fileSize: -1
      };
      const resNegative = shouldInterceptDownload({ item: itemNegative, settings: baseSettings });
      assert.equal(resNegative.intercept, true);
      assert.equal(resNegative.reason, "allowed");
    });

    test("4. non-listed extension is skipped", () => {
      const item = {
        id: 5,
        url: "https://example.com/page.html",
        filename: "page.html",
        totalBytes: 10 * 1024 * 1024
      };
      const res = shouldInterceptDownload({ item, settings: baseSettings });
      assert.equal(res.intercept, false);
      assert.equal(res.reason, "non_listed_extension");
    });

    test("5. alt-click bypass is skipped", () => {
      const now = Date.now();
      const item = {
        id: 6,
        url: "https://example.com/manual_download.zip",
        filename: "manual_download.zip",
        totalBytes: 20 * 1024 * 1024
      };
      const altBypasses = {
        "https://example.com/manual_download.zip": now - 1000 // 1s ago
      };
      const res = shouldInterceptDownload({
        item,
        settings: baseSettings,
        altBypasses,
        nowMs: now
      });
      assert.equal(res.intercept, false);
      assert.equal(res.reason, "alt_bypass");
    });

    test("6. disabled setting skips download", () => {
      const item = {
        id: 7,
        url: "https://example.com/file.zip",
        filename: "file.zip",
        totalBytes: 20 * 1024 * 1024
      };
      const res = shouldInterceptDownload({
        item,
        settings: { ...baseSettings, enabled: false }
      });
      assert.equal(res.intercept, false);
      assert.equal(res.reason, "disabled");
    });

    test("8. crdownload local name does not block URL extension intercept", () => {
      const item = {
        id: 9,
        url: "https://example.com/a.zip",
        filename: "Unconfirmed 12345.crdownload",
        totalBytes: 5 * 1024 * 1024
      };
      const res = shouldInterceptDownload({ item, settings: baseSettings });
      assert.equal(res.intercept, true);
      assert.equal(res.reason, "allowed");
      assert.equal(res.filename, "a.zip");
    });

    test("9. URL without listed extension is not intercepted without suggested name", () => {
      const item = {
        id: 10,
        url: "https://example.com/page",
        filename: "Unconfirmed 99.crdownload",
        totalBytes: 5 * 1024 * 1024
      };
      const res = shouldInterceptDownload({ item, settings: baseSettings });
      assert.equal(res.intercept, false);
      assert.equal(res.reason, "non_listed_extension");
    });

    test("10. suggested filename enables intercept when URL has no extension", () => {
      const item = {
        id: 11,
        url: "https://example.com/page",
        filename: "Unconfirmed 99.crdownload",
        totalBytes: 5 * 1024 * 1024
      };
      const res = shouldInterceptDownload({
        item,
        settings: baseSettings,
        suggestedFilename: "report.pdf"
      });
      assert.equal(res.intercept, true);
      assert.equal(res.reason, "allowed");
      assert.equal(res.filename, "report.pdf");
    });

    test("7. own download is skipped", () => {
      const extId = "abcdefghijklmnop";
      const item = {
        id: 8,
        url: "https://example.com/file.zip",
        filename: "file.zip",
        totalBytes: 20 * 1024 * 1024,
        byExtensionId: extId
      };
      const res = shouldInterceptDownload({
        item,
        settings: baseSettings,
        extensionId: extId
      });
      assert.equal(res.intercept, false);
      assert.equal(res.reason, "own_download");
    });
  });
});

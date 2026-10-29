import { test, describe } from "node:test";
import assert from "node:assert/strict";
// Portions derived from AB Download Manager (https://github.com/amir1376/ab-download-manager), Apache-2.0
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
  shouldInterceptDownload,
  isGithubRedirectUrl,
  isGithubReleaseUrl,
  isGoogleDriveUrl,
  isBinaryMime,
  isHtmlMime,
  isForceCaptureItem,
  isBlockedUrlScheme,
  HTML_RESPONSE_MESSAGE_AR,
  resolveInterceptFilename,
  isUrlDeduped,
  recordUrlDedupe,
  resetUrlDedupeCache,
  buildDedupeKey,
  urlDedupeCache,
  DEDUPE_WINDOW_MS,
  processDownloadCapture
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

    test("11. GitHub 302 signed finalUrl without extension uses release URL basename", () => {
      const signedFinal =
        "https://release-assets.githubusercontent.com/github-production-release-asset-2e65bee/12345678/abcdef0123456789?sp=r&sv=2018-11-09&sr=b&sig=signedtoken";
      const releaseUrl =
        "https://github.com/athar-studios/sayl/releases/download/v0.1.0/SAYL-Setup-x64.exe";

      assert.equal(isGithubRedirectUrl(signedFinal), true);

      const item = {
        id: 12,
        url: releaseUrl,
        finalUrl: signedFinal,
        filename: "Unconfirmed 12345.crdownload",
        totalBytes: 8 * 1024 * 1024
      };

      const res = shouldInterceptDownload({ item, settings: baseSettings });
      assert.equal(res.intercept, true);
      assert.equal(res.reason, "allowed");
      assert.equal(res.filename, "SAYL-Setup-x64.exe");
      assert.equal(res.targetUrl, signedFinal);
    });

    test("12. codeload.github.com redirect resolves filename from original url", () => {
      const signedFinal =
        "https://objects.githubusercontent.com/github-production-release-asset-2e65bee/999/aaaaaaaaaaaaaaaa?X-Amz-Algorithm=AWS4-HMAC-SHA256";
      const releaseUrl =
        "https://github.com/org/repo/releases/download/v1.0/app.zip";

      const item = {
        id: 13,
        url: releaseUrl,
        finalUrl: signedFinal,
        totalBytes: 0
      };

      const name = resolveInterceptFilename(item, undefined, DEFAULT_EXTENSIONS);
      assert.equal(name, "app.zip");

      const res = shouldInterceptDownload({ item, settings: baseSettings });
      assert.equal(res.intercept, true);
      assert.equal(res.filename, "app.zip");
    });

    test("13. own download is skipped (byExtensionId)", () => {
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

describe("isUrlDeduped (pure check, 3s window, keyed by downloadId::url)", () => {
  test("1. the check is pure: it never records by itself", () => {
    resetUrlDedupeCache();
    const url = "https://example.com/file.zip";
    assert.equal(isUrlDeduped(url), false);
    assert.equal(isUrlDeduped(url), false, "a pure check must not make the next check dedupe");
    assert.equal(urlDedupeCache.size, 0);
  });

  test("2. recorded URL is deduped inside the 3s window only", () => {
    resetUrlDedupeCache();
    const url = "https://example.com/file2.zip";
    const t0 = Date.now();
    recordUrlDedupe(url, t0);
    assert.equal(isUrlDeduped(url, t0 + 100), true);
    assert.equal(isUrlDeduped(url, t0 + 2900), true);
    assert.equal(isUrlDeduped(url, t0 + 3100), false, "after 3s window it must not be deduped");
  });

  test("3. different downloads (id) and different URLs are not deduped", () => {
    resetUrlDedupeCache();
    const t0 = Date.now();
    recordUrlDedupe("https://example.com/a.zip", t0, 1);
    assert.equal(isUrlDeduped("https://example.com/a.zip", t0 + 100, 2), false, "different download id bypasses dedupe");
    assert.equal(isUrlDeduped("https://example.com/b.zip", t0 + 100, 1), false, "different URL bypasses dedupe");
    assert.equal(isUrlDeduped("https://example.com/a.zip", t0 + 100, 1), true, "same id + same URL dedupes");
  });

  test("4. buildDedupeKey combines id and cleaned URL", () => {
    assert.equal(buildDedupeKey("https://example.com/a.zip?query=1", 123), "123::https://example.com/a.zip");
    assert.equal(buildDedupeKey("https://example.com/a.zip#frag", 123), "123::https://example.com/a.zip");
    assert.equal(buildDedupeKey("https://example.com/a.zip", undefined), "https://example.com/a.zip");
  });
});

describe("extension guards and download capture pipeline", () => {
  const baseSettings = { enabled: true, minSizeMb: 1, extensions: DEFAULT_EXTENSIONS };
  const baseAltBypasses = {};

  test("14. normal new download is captured and allowed (guards do not skip it)", () => {
    resetUrlDedupeCache();
    const now = Date.now();
    const item = {
      id: 201,
      url: "https://example.com/fresh-release.zip",
      filename: "fresh-release.zip",
      totalBytes: 15 * 1024 * 1024,
      startTime: new Date(now - 1000).toISOString() // 1 second ago
    };
    const res = shouldInterceptDownload({
      item,
      settings: baseSettings,
      altBypasses: baseAltBypasses,
      extensionId: "our-extension-id",
      nowMs: now
    });
    assert.equal(res.intercept, true);
    assert.equal(res.reason, "allowed");
  });

  test("15. captured download => cancel called immediately, browser download not kept", async () => {
    resetUrlDedupeCache();
    const cancelledIds = [];
    const erasedIds = [];
    const fakeDownloadsApi = {
      cancel: async (id) => { cancelledIds.push(id); },
      erase: async (query) => { erasedIds.push(query.id); },
      download: async () => {}
    };
    const fakeSendCapture = async () => ({ ok: true, response: { task_id: "sayl-task-99" } });

    const item = {
      id: 202,
      url: "https://example.com/package.tar.gz",
      filename: "package.tar.gz",
      totalBytes: 50 * 1024 * 1024
    };

    const res = await processDownloadCapture({
      item,
      settings: baseSettings,
      downloadsApi: fakeDownloadsApi,
      sendCaptureFn: fakeSendCapture
    });

    assert.equal(res.intercept, true);
    assert.equal(res.cancelled, true, "chrome.downloads.cancel must be called immediately");
    assert.equal(res.erased, true, "chrome.downloads.erase must be called immediately");
    assert.deepEqual(cancelledIds, [202]);
    assert.deepEqual(erasedIds, [202]);
    assert.equal(res.redownloaded, false, "must not re-download when host succeeds");
  });

  test("16. host send fails => re-downloaded in browser once with marker (give back once)", async () => {
    resetUrlDedupeCache();
    const redownloadedUrls = [];
    const fakeDownloadsApi = {
      cancel: async () => {},
      erase: async () => {},
      download: async (opts) => { redownloadedUrls.push(opts.url); }
    };
    const fakeSendCaptureFail = async () => ({
      ok: false,
      error: { code: "HOST_UNREACHABLE", message: "Sayl app not running" }
    });
    const givenBackUrls = new Set();

    const item = {
      id: 203,
      url: "https://example.com/driver.msi",
      filename: "driver.msi",
      totalBytes: 25 * 1024 * 1024
    };

    // First attempt: host fails
    const res = await processDownloadCapture({
      item,
      settings: baseSettings,
      givenBackUrls,
      downloadsApi: fakeDownloadsApi,
      sendCaptureFn: fakeSendCaptureFail
    });

    assert.equal(res.intercept, true);
    assert.equal(res.cancelled, true, "original download must be cancelled first");
    assert.equal(res.redownloaded, true, "must trigger browser re-download once");
    assert.deepEqual(redownloadedUrls, ["https://example.com/driver.msi"]);
    assert.equal(givenBackUrls.has("https://example.com/driver.msi"), true);

    // Second event arriving from browser re-download: must be skipped and allowed in browser
    const retryItem = {
      id: 204,
      url: "https://example.com/driver.msi",
      filename: "driver.msi",
      totalBytes: 25 * 1024 * 1024
    };
    const retryRes = shouldInterceptDownload({
      item: retryItem,
      settings: baseSettings,
      givenBackUrls
    });
    assert.equal(retryRes.intercept, false);
    assert.equal(retryRes.reason, "given_back_to_browser");
  });

  test("17. restored download (byExtensionId from other extension) is skipped", () => {
    const item = {
      id: 205,
      url: "https://example.com/other-ext.zip",
      filename: "other-ext.zip",
      totalBytes: 10 * 1024 * 1024,
      byExtensionId: "some-other-extension-id"
    };
    const res = shouldInterceptDownload({
      item,
      settings: baseSettings,
      extensionId: "our-extension-id"
    });
    assert.equal(res.intercept, false);
    assert.equal(res.reason, "restored_download");
  });

  test("18. download older than 30s is skipped", () => {
    const now = Date.now();
    const item = {
      id: 206,
      url: "https://example.com/old-file.zip",
      filename: "old-file.zip",
      totalBytes: 10 * 1024 * 1024,
      startTime: new Date(now - 45_000).toISOString() // 45s ago
    };
    const res = shouldInterceptDownload({
      item,
      settings: baseSettings,
      nowMs: now
    });
    assert.equal(res.intercept, false);
    assert.equal(res.reason, "old_download");
  });

  test("19. same download is deduped within 3s only after an actual intercept (pure check)", () => {
    resetUrlDedupeCache();
    const now = Date.now();
    const url = "https://example.com/dedupe-file.zip";
    const item = {
      id: 207,
      url,
      filename: "dedupe-file.zip",
      totalBytes: 10 * 1024 * 1024
    };

    // Two decision calls for the SAME download never dedupe each other,
    // because the check is pure and records nothing.
    const first = shouldInterceptDownload({
      item,
      settings: baseSettings,
      nowMs: now,
      checkDedupe: true
    });
    assert.equal(first.intercept, true);

    const second = shouldInterceptDownload({
      item,
      settings: baseSettings,
      nowMs: now + 50,
      checkDedupe: true
    });
    // 50 ms is the onCreated/onDeterminingFilename race window.
    assert.equal(second.intercept, true, "the 50ms race must no longer dedupe");

    // Now simulate the real record that happens after the intercept.
    recordUrlDedupe(url, now, item.id);

    const third = shouldInterceptDownload({
      item: { ...item, id: 207 },
      settings: baseSettings,
      nowMs: now + 1000,
      checkDedupe: true
    });
    assert.equal(third.intercept, false);
    assert.equal(third.reason, "url_deduped");
  });

  test("20. intercepted => cancel before any other await, suggest not called", async () => {
    resetUrlDedupeCache();
    const callOrder = [];
    let suggestCalled = false;
    const suggest = () => { suggestCalled = true; };

    const fakeDownloadsApi = {
      cancel: async (id) => {
        callOrder.push(`cancel:${id}`);
      },
      removeFile: async (id) => {
        callOrder.push(`removeFile:${id}`);
      },
      erase: async ({ id }) => {
        callOrder.push(`erase:${id}`);
      },
      download: async () => {
        callOrder.push("download");
      }
    };

    const fakeSendCapture = async () => {
      callOrder.push("sendCapture");
      return { ok: true, response: { task_id: "test-task-1" } };
    };

    const item = {
      id: 301,
      url: "https://example.com/software.exe",
      filename: "software.exe",
      totalBytes: 15 * 1024 * 1024
    };

    // When intercepted, suggest must NOT be called
    const decision = shouldInterceptDownload({ item, settings: baseSettings });
    assert.equal(decision.intercept, true);
    if (!decision.intercept) {
      suggest({ filename: item.filename });
    }
    assert.equal(suggestCalled, false, "suggest must not be called when intercepting");

    const res = await processDownloadCapture({
      item,
      settings: baseSettings,
      downloadsApi: fakeDownloadsApi,
      sendCaptureFn: fakeSendCapture
    });

    assert.equal(res.intercept, true);
    assert.equal(res.cancelled, true);
    assert.equal(callOrder[0], "cancel:301", "cancel must be the very first operation");
    assert.equal(callOrder[1], "erase:301", "erase must follow cancel");
    assert.equal(callOrder[2], "sendCapture", "sendCapture must follow erase");
  });

  test("21. timeout => no browser re-download", async () => {
    resetUrlDedupeCache();
    let downloadCalled = false;
    const fakeDownloadsApi = {
      cancel: async () => {},
      erase: async () => {},
      download: async () => {
        downloadCalled = true;
      }
    };

    // Host times out after 10s
    const fakeSendCaptureTimeout = async () => ({
      ok: false,
      error: { code: "TIMEOUT", message: "SAYL host timed out after 10s" }
    });

    const item = {
      id: 302,
      url: "https://example.com/large-archive.zip",
      filename: "large-archive.zip",
      totalBytes: 50 * 1024 * 1024
    };

    const res = await processDownloadCapture({
      item,
      settings: baseSettings,
      downloadsApi: fakeDownloadsApi,
      sendCaptureFn: fakeSendCaptureTimeout
    });

    assert.equal(res.intercept, true);
    assert.equal(res.cancelled, true);
    assert.equal(res.redownloaded, false, "timeout must never trigger browser re-download");
    assert.equal(downloadCalled, false, "chrome.downloads.download must not be called on timeout");
  });

  test("22. host error reply => no browser re-download", async () => {
    resetUrlDedupeCache();
    let downloadCalled = false;
    const fakeDownloadsApi = {
      cancel: async () => {},
      erase: async () => {},
      download: async () => {
        downloadCalled = true;
      }
    };

    const fakeSendCaptureError = async () => ({
      ok: false,
      error: { code: "HOST_ERROR", message: "User rejected download" }
    });

    const item = {
      id: 303,
      url: "https://example.com/rejected.iso",
      filename: "rejected.iso",
      totalBytes: 100 * 1024 * 1024
    };

    const res = await processDownloadCapture({
      item,
      settings: baseSettings,
      downloadsApi: fakeDownloadsApi,
      sendCaptureFn: fakeSendCaptureError
    });

    assert.equal(res.intercept, true);
    assert.equal(res.redownloaded, false, "host error reply must never trigger browser re-download");
    assert.equal(downloadCalled, false);
  });

  test("23. completed item => removeFile before erase", async () => {
    resetUrlDedupeCache();
    const callOrder = [];
    const fakeDownloadsApi = {
      cancel: async (id) => { callOrder.push(`cancel:${id}`); },
      removeFile: async (id) => { callOrder.push(`removeFile:${id}`); },
      erase: async ({ id }) => { callOrder.push(`erase:${id}`); },
      download: async () => {}
    };

    const fakeSendCapture = async () => ({
      ok: true,
      response: { task_id: "complete-task-1" }
    });

    const item = {
      id: 304,
      url: "https://example.com/already-done.bin",
      filename: "already-done.bin",
      totalBytes: 5 * 1024 * 1024,
      state: "complete"
    };

    const res = await processDownloadCapture({
      item,
      settings: baseSettings,
      downloadsApi: fakeDownloadsApi,
      sendCaptureFn: fakeSendCapture
    });

    assert.equal(res.intercept, true);
    assert.equal(res.removedFile, true);
    assert.deepEqual(callOrder, ["cancel:304", "removeFile:304", "erase:304"]);
  });
});

describe("GitHub / Google Drive / binary capture (no listed extension required)", () => {
  const baseSettings = { enabled: true, minSizeMb: 1, extensions: DEFAULT_EXTENSIONS };

  test("24. GitHub release URL with a UUID asset and no extension is captured", () => {
    resetUrlDedupeCache();
    const uuid = "1a2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d";
    const releaseUrl = `https://github.com/athar-studios/sayl/releases/download/v0.2.0/${uuid}`;
    const item = {
      id: 401,
      url: releaseUrl,
      filename: "Unconfirmed 71009.crdownload",
      totalBytes: 0
    };

    assert.equal(isGithubReleaseUrl(releaseUrl), true, "release URL must be recognized");

    const res = shouldInterceptDownload({ item, settings: baseSettings, checkDedupe: true });
    assert.equal(res.intercept, true);
    assert.equal(res.reason, "allowed");
    assert.equal(res.filename, uuid, "filename falls back to the last URL path segment");
    assert.equal(res.targetUrl, releaseUrl);
  });

  test("25. codeload.github.com zip with no size is captured", () => {
    resetUrlDedupeCache();
    const archiveUrl = "https://codeload.github.com/athar-studios/sayl/zip/refs/heads/main";
    const item = {
      id: 402,
      url: archiveUrl,
      filename: "main",
      totalBytes: 0,
      fileSize: 0
    };

    assert.equal(isGithubReleaseUrl(archiveUrl), true, "codeload is a GitHub release/archive host");

    const res = shouldInterceptDownload({ item, settings: baseSettings, checkDedupe: true });
    assert.equal(res.intercept, true);
    assert.equal(res.reason, "allowed", "unknown size must not block a forced capture");
    assert.equal(res.size, 0);
  });

  test("26. drive.usercontent.google.com download is captured (unknown size, no extension)", () => {
    resetUrlDedupeCache();
    const finalUrl =
      "https://drive.usercontent.google.com/download?id=1AbCdEfGhIjKlMnOp&export=download&confirm=t";
    const sourceUrl = "https://drive.google.com/file/d/1AbCdEfGhIjKlMnOp/view";
    const item = {
      id: 403,
      url: sourceUrl,
      finalUrl,
      filename: "Unconfirmed 55231.crdownload",
      totalBytes: 0
    };

    assert.equal(isGoogleDriveUrl(finalUrl), true);
    assert.equal(isGoogleDriveUrl(sourceUrl), true);

    const res = shouldInterceptDownload({
      item,
      settings: baseSettings,
      suggestedFilename: "quarterly-report.pdf",
      checkDedupe: true
    });
    assert.equal(res.intercept, true);
    assert.equal(res.reason, "allowed");
    assert.equal(res.filename, "quarterly-report.pdf", "onDeterminingFilename name wins");
    assert.equal(res.targetUrl, finalUrl);
  });

  test("27. docs.google.com uc?export=download is captured, plain docs page is not", () => {
    const ucUrl = "https://docs.google.com/uc?export=download&id=1AbCdEfGhIjKlMnOp";
    assert.equal(isGoogleDriveUrl(ucUrl), true);

    const docPage = "https://docs.google.com/document/d/1AbCdEfGhIjKlMnOp/edit";
    assert.equal(isGoogleDriveUrl(docPage), false);

    const ucItem = { id: 404, url: ucUrl, totalBytes: 0 };
    const ucRes = shouldInterceptDownload({ item: ucItem, settings: baseSettings });
    assert.equal(ucRes.intercept, true);

    const docItem = {
      id: 405,
      url: docPage,
      filename: "edit",
      totalBytes: 0,
      mime: "text/html"
    };
    const docRes = shouldInterceptDownload({ item: docItem, settings: baseSettings });
    assert.equal(docRes.intercept, false);
    assert.equal(docRes.reason, "html_response");
  });

  test("28. binary MIME bypasses the extension allowlist", () => {
    const item = {
      id: 406,
      url: "https://cdn.example.com/download/8f3c2b1a",
      filename: "8f3c2b1a",
      totalBytes: 0,
      mime: "application/octet-stream"
    };

    assert.equal(isBinaryMime("application/octet-stream"), true);
    assert.equal(isBinaryMime("application/x-msdownload"), true);
    assert.equal(isBinaryMime("application/zip"), true);
    assert.equal(isBinaryMime("application/x-rar-compressed"), true);
    assert.equal(isBinaryMime("video/mp4"), true);
    assert.equal(isBinaryMime("text/html"), false);
    assert.equal(isBinaryMime(""), false);

    const res = shouldInterceptDownload({ item, settings: baseSettings });
    assert.equal(res.intercept, true);
    assert.equal(res.reason, "allowed");
    assert.equal(res.forced, true);
  });

  test("29. HTML response is never intercepted and never loops", () => {
    const item = {
      id: 407,
      url: "https://drive.google.com/file/d/1AbCdEfGhIjKlMnOp/view",
      filename: "file",
      totalBytes: 0,
      mime: "text/html; charset=utf-8"
    };

    assert.equal(isHtmlMime("text/html; charset=utf-8"), true);

    const res = shouldInterceptDownload({ item, settings: baseSettings });
    assert.equal(res.intercept, false);
    assert.equal(res.reason, "html_response");
    assert.equal(res.message, HTML_RESPONSE_MESSAGE_AR);
  });

  test("30. blob:, data: and filesystem: URLs are never intercepted", () => {
    const settings = { enabled: true, minSizeMb: 1, extensions: DEFAULT_EXTENSIONS };

    for (const url of [
      "blob:https://example.com/6f1a2b3c-4d5e-6f70-8192-a3b4c5d6e7f8",
      "data:application/octet-stream;base64,AAAA",
      "filesystem:https://example.com/temporary/file.zip"
    ]) {
      assert.equal(isBlockedUrlScheme(url), true, `${url} must be a blocked scheme`);
      const res = shouldInterceptDownload({
        item: { id: 408, url, filename: "file.zip", totalBytes: 50 * 1024 * 1024 },
        settings
      });
      assert.equal(res.intercept, false, `${url} must not be intercepted`);
      assert.equal(res.reason, "non_http_url");
    }

    assert.equal(isBlockedUrlScheme("https://example.com/file.zip"), false);
  });

  test("31. a blob: source URL with an http finalUrl is still not intercepted", () => {
    const res = shouldInterceptDownload({
      item: {
        id: 409,
        url: "blob:https://example.com/abc-123",
        finalUrl: "https://example.com/file.zip",
        filename: "file.zip",
        totalBytes: 50 * 1024 * 1024
      },
      settings: { enabled: true, minSizeMb: 1, extensions: DEFAULT_EXTENSIONS }
    });
    assert.equal(res.intercept, false);
    assert.equal(res.reason, "non_http_url");
  });

  test("32. onCreated/onDeterminingFilename race no longer dedupes", () => {
    resetUrlDedupeCache();
    const now = Date.now();
    const item = {
      id: 410,
      url: "https://github.com/athar-studios/sayl/releases/download/v0.2.0/sayl-setup.exe",
      filename: "Unconfirmed 33119.crdownload",
      totalBytes: 0,
      mime: "application/octet-stream"
    };

    const fromDetermining = shouldInterceptDownload({
      item,
      settings: baseSettings,
      nowMs: now,
      checkDedupe: true
    });
    const fromCreated = shouldInterceptDownload({
      item,
      settings: baseSettings,
      nowMs: now + 50,
      checkDedupe: true
    });

    assert.equal(fromDetermining.intercept, true);
    assert.equal(fromCreated.intercept, true, "the 50ms onCreated race must not dedupe");
    assert.equal(fromCreated.reason, "allowed");

    // After the real intercept is recorded, a third event for the same download is deduped.
    recordUrlDedupe(fromDetermining.targetUrl, now, item.id);
    const afterIntercept = shouldInterceptDownload({
      item,
      settings: baseSettings,
      nowMs: now + 100,
      checkDedupe: true
    });
    assert.equal(afterIntercept.intercept, false);
    assert.equal(afterIntercept.reason, "url_deduped");
  });

  test("33. the same URL twice more than 3s apart is captured both times", () => {
    resetUrlDedupeCache();
    const url = "https://example.com/installer.exe";
    const firstItem = { id: 411, url, filename: "installer.exe", totalBytes: 20 * 1024 * 1024 };
    const t0 = Date.now();

    const first = shouldInterceptDownload({
      item: firstItem,
      settings: baseSettings,
      nowMs: t0,
      checkDedupe: true
    });
    assert.equal(first.intercept, true);
    recordUrlDedupe(url, t0, firstItem.id);

    // Same download id, just past the 3s window.
    const later = shouldInterceptDownload({
      item: firstItem,
      settings: baseSettings,
      nowMs: t0 + DEDUPE_WINDOW_MS + 100,
      checkDedupe: true
    });
    assert.equal(later.intercept, true, ">3s later must be captured again");

    // A second, genuinely new download of the same URL is never deduped either.
    const second = shouldInterceptDownload({
      item: { ...firstItem, id: 412 },
      settings: baseSettings,
      nowMs: t0 + 200,
      checkDedupe: true
    });
    assert.equal(second.intercept, true);
  });

  test("34. forced captures still respect a known size below the minimum", () => {
    const item = {
      id: 413,
      url: "https://github.com/org/repo/releases/download/v1.0/tiny.txtasset",
      filename: "tiny.txtasset",
      totalBytes: 2 * 1024,
      mime: "application/octet-stream"
    };
    const res = shouldInterceptDownload({
      item,
      settings: { enabled: true, minSizeMb: 1, extensions: DEFAULT_EXTENSIONS }
    });
    assert.equal(res.intercept, false);
    assert.equal(res.reason, "too_small");
  });

  test("35. isForceCaptureItem covers releases, Drive and binary MIME only", () => {
    assert.equal(
      isForceCaptureItem({ url: "https://github.com/org/repo/archive/refs/tags/v1.0.zip" }),
      true
    );
    assert.equal(
      isForceCaptureItem({ url: "https://drive.usercontent.google.com/download?id=1&export=download" }),
      true
    );
    assert.equal(isForceCaptureItem({ url: "https://example.com/page", mime: "application/zip" }), true);
    assert.equal(isForceCaptureItem({ url: "https://example.com/page" }), false);
    assert.equal(isForceCaptureItem({ url: "https://example.com/page", mime: "text/html" }), false);
    assert.equal(isForceCaptureItem(null), false);
  });

  test("36. the new archive/document extensions are in DEFAULT_EXTENSIONS", () => {
    for (const ext of [
      "docx", "xlsx", "pptx", "pdf", "epub", "rar", "7z", "tgz", "zst",
      "deb", "rpm", "appimage", "apk", "jar", "iso", "mp4", "mkv", "mp3", "flac"
    ]) {
      assert.equal(DEFAULT_EXTENSIONS.includes(ext), true, `${ext} must be listed`);
      assert.equal(matchesExtension(`file.${ext}`, DEFAULT_EXTENSIONS), true);
    }
  });

  test("37. HTML failure from the host is reported in Arabic and never re-downloaded", async () => {
    resetUrlDedupeCache();
    let downloadCalled = false;
    const fakeDownloadsApi = {
      cancel: async () => {},
      erase: async () => {},
      download: async () => {
        downloadCalled = true;
      }
    };
    const fakeSendCapture = async () => ({
      ok: false,
      error: { code: "html_response", message: "server returned text/html" }
    });

    const res = await processDownloadCapture({
      item: {
        id: 414,
        url: "https://example.com/archive.zip",
        filename: "archive.zip",
        totalBytes: 10 * 1024 * 1024
      },
      settings: baseSettings,
      downloadsApi: fakeDownloadsApi,
      sendCaptureFn: fakeSendCapture
    });

    assert.equal(res.message, HTML_RESPONSE_MESSAGE_AR);
    assert.equal(res.redownloaded, false, "an HTML failure must never be given back");
    assert.equal(downloadCalled, false);
  });

  test("38. Google Drive virus-scan confirm page (HTML) is rejected, actual download after 'Download anyway' is captured", () => {
    resetUrlDedupeCache();
    // The confirm page returns HTML — must not be intercepted
    const confirmPageItem = {
      id: 501,
      url: "https://drive.google.com/uc?export=download&id=1AbCdEfGhIjKlMnOp",
      finalUrl: "https://drive.usercontent.google.com/download?id=1AbCdEfGhIjKlMnOp&export=download&confirm=t",
      filename: "confirm.html",
      totalBytes: 0,
      mime: "text/html; charset=utf-8"
    };
    const confirmRes = shouldInterceptDownload({ item: confirmPageItem, settings: baseSettings });
    assert.equal(confirmRes.intercept, false);
    assert.equal(confirmRes.reason, "html_response");
    assert.equal(confirmRes.message, HTML_RESPONSE_MESSAGE_AR);

    // The actual download after clicking "Download anyway" has binary MIME
    const actualDownloadItem = {
      id: 502,
      url: "https://drive.google.com/uc?export=download&id=1AbCdEfGhIjKlMnOp",
      finalUrl: "https://drive.usercontent.google.com/download?id=1AbCdEfGhIjKlMnOp&export=download&confirm=t&uuid=abc123",
      filename: "quarterly-report.pdf",
      totalBytes: 5 * 1024 * 1024,
      mime: "application/pdf"
    };
    const actualRes = shouldInterceptDownload({
      item: actualDownloadItem,
      settings: baseSettings,
      suggestedFilename: "quarterly-report.pdf"
    });
    assert.equal(actualRes.intercept, true);
    assert.equal(actualRes.reason, "allowed");
    assert.equal(actualRes.filename, "quarterly-report.pdf");
    assert.equal(actualRes.forced, true, "Drive downloads are forced captures");
  });

  test("39. drive.usercontent.google.com with confirm=t is recognized as forced capture", () => {
    const urls = [
      "https://drive.usercontent.google.com/download?id=123&export=download&confirm=t",
      "https://drive.usercontent.google.com/download?id=123&export=download&confirm=yes",
      "https://drive.usercontent.google.com/uc?id=123&export=download"
    ];
    for (const url of urls) {
      assert.equal(isGoogleDriveUrl(url), true, `${url} must be recognized as Google Drive`);
      const item = { id: 1, url, finalUrl: url, totalBytes: 0 };
      assert.equal(isForceCaptureItem(item), true, `${url} must be a forced capture`);
    }
  });

  test("40. immediate consecutive downloads of the same URL (different IDs) are both captured", () => {
    resetUrlDedupeCache();
    const url = "https://example.com/package.zip";
    const baseSettings = { enabled: true, minSizeMb: 1, extensions: DEFAULT_EXTENSIONS };
    const t0 = Date.now();

    // First download
    const firstItem = { id: 601, url, filename: "package.zip", totalBytes: 10 * 1024 * 1024 };
    const first = shouldInterceptDownload({ item: firstItem, settings: baseSettings, nowMs: t0, checkDedupe: true });
    assert.equal(first.intercept, true, "first download must be intercepted");
    recordUrlDedupe(url, t0, firstItem.id);

    // Second download, same URL, different ID, 100ms later (within 3s window)
    const secondItem = { id: 602, url, filename: "package.zip", totalBytes: 10 * 1024 * 1024 };
    const second = shouldInterceptDownload({ item: secondItem, settings: baseSettings, nowMs: t0 + 100, checkDedupe: true });
    assert.equal(second.intercept, true, "second download with different ID must NOT be deduped");
    assert.equal(second.reason, "allowed");

    // Third download, same URL, same ID as first (should not happen in practice, but test dedupe)
    const thirdItem = { id: 601, url, filename: "package.zip", totalBytes: 10 * 1024 * 1024 };
    const third = shouldInterceptDownload({ item: thirdItem, settings: baseSettings, nowMs: t0 + 200, checkDedupe: true });
    assert.equal(third.intercept, false, "same ID + same URL within window must be deduped");
    assert.equal(third.reason, "url_deduped");
  });

  test("41. redirect chain: github.com -> objects.githubusercontent.com preserves original filename", () => {
    const releaseUrl = "https://github.com/owner/repo/releases/download/v1.0/app-v1.0.exe";
    const redirect1 = "https://objects.githubusercontent.com/github-production-release-asset-123/456/abc?X-Amz-Signature=sig1";
    const redirect2 = "https://objects.githubusercontent.com/github-production-release-asset-123/456/abc?X-Amz-Signature=sig2&response-content-disposition=attachment%3Bfilename%3Dapp-v1.0.exe";

    const item = {
      id: 701,
      url: releaseUrl,
      finalUrl: redirect2,
      filename: "Unconfirmed 123.crdownload",
      totalBytes: 20 * 1024 * 1024
    };

    assert.equal(isGithubReleaseUrl(releaseUrl), true);
    assert.equal(isGithubRedirectUrl(redirect1), true);
    assert.equal(isGithubRedirectUrl(redirect2), true);

    const res = shouldInterceptDownload({ item, settings: baseSettings });
    assert.equal(res.intercept, true);
    assert.equal(res.reason, "allowed");
    assert.equal(res.targetUrl, redirect2);
    // Filename should come from the original release URL since final URL has no usable extension
    assert.equal(res.filename, "app-v1.0.exe");
  });

  test("42. Content-Disposition filename from onDeterminingFilename takes priority over URL", () => {
    const item = {
      id: 801,
      url: "https://cdn.example.com/download/abc123",
      finalUrl: "https://cdn.example.com/download/abc123?token=xyz",
      filename: "my-custom-name.zip", // Browser-determined filename from Content-Disposition
      totalBytes: 15 * 1024 * 1024
    };

    const res = shouldInterceptDownload({
      item,
      settings: baseSettings,
      suggestedFilename: "my-custom-name.zip"
    });
    assert.equal(res.intercept, true);
    assert.equal(res.filename, "my-custom-name.zip", "Content-Disposition filename wins");
  });

  test("43. GitHub codeload archive (zip/tar.gz) with no size is captured", () => {
    resetUrlDedupeCache();
    const urls = [
      "https://codeload.github.com/owner/repo/zip/refs/heads/main",
      "https://codeload.github.com/owner/repo/tar.gz/refs/tags/v1.0",
      "https://codeload.github.com/owner/repo/legacy.zip/refs/heads/main"
    ];
    for (const url of urls) {
      assert.equal(isGithubReleaseUrl(url), true, `${url} must be recognized as GitHub release/archive`);
      const item = { id: 901, url, filename: "main", totalBytes: 0, fileSize: 0 };
      const res = shouldInterceptDownload({ item, settings: baseSettings });
      assert.equal(res.intercept, true, `${url} must be intercepted`);
      assert.equal(res.reason, "allowed");
      assert.equal(res.forced, true);
    }
  });

  test("44. release-assets.githubusercontent.com redirect is recognized", () => {
    const url = "https://release-assets.githubusercontent.com/github-production-release-asset-2e65bee/12345678/abcdef?X-Amz-Signature=sig";
    assert.equal(isGithubRedirectUrl(url), true);
    assert.equal(isGithubReleaseUrl(url), true);

    const item = {
      id: 1001,
      url: "https://github.com/owner/repo/releases/download/v1.0/file.bin",
      finalUrl: url,
      filename: "Unconfirmed 456.crdownload",
      totalBytes: 0
    };
    const res = shouldInterceptDownload({ item, settings: baseSettings });
    assert.equal(res.intercept, true);
    assert.equal(res.forced, true);
  });

  test("45. duplicate download given back to browser is not re-intercepted", () => {
    resetUrlDedupeCache();
    const givenBackUrls = new Set();
    const url = "https://example.com/driver.msi";
    const item = { id: 1101, url, filename: "driver.msi", totalBytes: 25 * 1024 * 1024 };
    const t0 = Date.now();

    // First attempt: host unreachable, given back
    const first = shouldInterceptDownload({ item, settings: baseSettings, nowMs: t0, givenBackUrls, checkDedupe: true });
    assert.equal(first.intercept, true);

    // Simulate host unreachable -> given back
    givenBackUrls.add(url);

    // Second event from browser re-download
    const retryItem = { id: 1102, url, filename: "driver.msi", totalBytes: 25 * 1024 * 1024 };
    const retry = shouldInterceptDownload({ item: retryItem, settings: baseSettings, nowMs: t0 + 500, givenBackUrls });
    assert.equal(retry.intercept, false);
    assert.equal(retry.reason, "given_back_to_browser");

    // Third event after given-back URL is cleared (simulating user clicking again later)
    givenBackUrls.delete(url);
    const third = shouldInterceptDownload({ item: { ...retryItem, id: 1103 }, settings: baseSettings, nowMs: t0 + 1000, givenBackUrls, checkDedupe: true });
    assert.equal(third.intercept, true, "after given-back is cleared, download must be captured again");
  });
});

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
  shouldInterceptDownload,
  isGithubRedirectUrl,
  resolveInterceptFilename,
  isUrlDeduped,
  resetUrlDedupeCache,
  urlDedupeCache,
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

describe("isUrlDeduped", () => {
  test("1. same URL within 15s is deduped", () => {
    resetUrlDedupeCache();
    const url = "https://example.com/file.zip";
    assert.equal(isUrlDeduped(url), false, "first time should not be deduped");
    assert.equal(isUrlDeduped(url), true, "second time within 15s should be deduped");
  });

  test("2. same URL after 15s is not deduped", () => {
    resetUrlDedupeCache();
    const url = "https://example.com/file2.zip";
    assert.equal(isUrlDeduped(url), false);
    // Simulate time passing by manipulating the cache directly
    urlDedupeCache.set(url, Date.now() - 20_000);
    assert.equal(isUrlDeduped(url), false, "after 15s should not be deduped");
  });

  test("3. different URLs are not deduped", () => {
    resetUrlDedupeCache();
    assert.equal(isUrlDeduped("https://example.com/a.zip"), false);
    assert.equal(isUrlDeduped("https://example.com/b.zip"), false);
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

  test("19. same-URL dedupe within 15s is skipped when checkDedupe is true", () => {
    resetUrlDedupeCache();
    const now = Date.now();
    const url = "https://example.com/dedupe-file.zip";
    const item = {
      id: 207,
      url,
      filename: "dedupe-file.zip",
      totalBytes: 10 * 1024 * 1024
    };

    const first = shouldInterceptDownload({
      item,
      settings: baseSettings,
      nowMs: now,
      checkDedupe: true
    });
    assert.equal(first.intercept, true);

    // Record dedupe for this URL
    urlDedupeCache.set(url, now);

    // Second download for same URL within 15s
    const second = shouldInterceptDownload({
      item: { ...item, id: 208 },
      settings: baseSettings,
      nowMs: now + 5000,
      checkDedupe: true
    });
    assert.equal(second.intercept, false);
    assert.equal(second.reason, "url_deduped");
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

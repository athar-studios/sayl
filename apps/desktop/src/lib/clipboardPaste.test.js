import { describe, it, expect } from "vitest";
import {
  extractUrls,
  isEditableTarget,
  matchPasteShortcut,
  shouldHandleClipboardPaste,
  splitClipboardIntoActions,
} from "./clipboardPaste.js";

describe("Clipboard paste helpers", () => {
  describe("extractUrls", () => {
    it("extracts a single URL", () => {
      expect(extractUrls("https://example.com/file.zip")).toEqual([
        "https://example.com/file.zip",
      ]);
    });

    it("extracts a one-per-line list in order", () => {
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

    it("extracts whitespace-separated URLs (spaces and tabs, with CRLF)", () => {
      const text =
        "  https://example.com/1.zip \t https://example.com/2.zip \r\n https://example.com/3.zip  ";

      expect(extractUrls(text)).toEqual([
        "https://example.com/1.zip",
        "https://example.com/2.zip",
        "https://example.com/3.zip",
      ]);
    });

    it("trims surrounding whitespace from each token", () => {
      expect(extractUrls("   \n\t https://example.com/file.zip \t\n  ")).toEqual([
        "https://example.com/file.zip",
      ]);
    });

    it("rejects schemes other than http, https, and ftp", () => {
      const text = [
        "javascript:alert(1)",
        "data:text/plain;base64,SGVsbG8=",
        "file:///C:/Windows/file.iso",
        "mailto:user@example.com",
        "chrome://settings",
        "https://example.com/kept.zip",
      ].join("\n");

      expect(extractUrls(text)).toEqual(["https://example.com/kept.zip"]);
    });

    it("rejects URLs without a host", () => {
      // Scheme without any authority → `new URL()` throws, so nothing is returned.
      expect(extractUrls("http://")).toEqual([]);
      expect(extractUrls("https://")).toEqual([]);
      expect(extractUrls("ftp://")).toEqual([]);
    });

    it("rejects non-URL prose tokens", () => {
      expect(extractUrls("hello world, not a url")).toEqual([]);
      expect(extractUrls("example.com/file.zip")).toEqual([]);
      expect(extractUrls("www.example.com")).toEqual([]);
    });

    it("de-duplicates while preserving first-seen order", () => {
      const text = [
        "https://example.com/a.zip",
        "https://example.com/b.zip",
        "https://example.com/a.zip",
        "https://example.com/b.zip",
      ].join("\n");

      expect(extractUrls(text)).toEqual([
        "https://example.com/a.zip",
        "https://example.com/b.zip",
      ]);
    });

    it("returns an empty array for empty and whitespace-only input", () => {
      expect(extractUrls("")).toEqual([]);
      expect(extractUrls("   \t\r\n\n ")).toEqual([]);
    });

    it("returns an empty array for non-string input without throwing", () => {
      expect(extractUrls(null)).toEqual([]);
      expect(extractUrls(undefined)).toEqual([]);
      expect(extractUrls(42)).toEqual([]);
      expect(extractUrls({})).toEqual([]);
      expect(extractUrls(["https://example.com/a.zip"])).toEqual([]);
    });

    it("caps the result at 100 entries", () => {
      const many = Array.from({ length: 150 }, (_, i) => `https://example.com/f${i}.zip`);
      const result = extractUrls(many.join("\n"));

      expect(result).toHaveLength(100);
      expect(result[0]).toBe("https://example.com/f0.zip");
      expect(result[99]).toBe("https://example.com/f99.zip");
    });

    it("keeps IPv4 hosts and ports intact", () => {
      expect(extractUrls("http://127.0.0.1:8080/file.zip")).toEqual([
        "http://127.0.0.1:8080/file.zip",
      ]);
    });
  });

  describe("isEditableTarget", () => {
    it("returns true for text input, textarea, and select", () => {
      expect(isEditableTarget({ tagName: "INPUT", type: "text" })).toBe(true);
      expect(isEditableTarget({ tagName: "TEXTAREA" })).toBe(true);
      expect(isEditableTarget({ tagName: "SELECT" })).toBe(true);
    });

    it("treats an input with no type as a text input", () => {
      expect(isEditableTarget({ tagName: "INPUT" })).toBe(true);
      expect(isEditableTarget({ tagName: "INPUT", type: "" })).toBe(true);
    });

    it("accepts lowercase tag names (duck-typed elements)", () => {
      expect(isEditableTarget({ tagName: "input", type: "text" })).toBe(true);
      expect(isEditableTarget({ tagName: "textarea" })).toBe(true);
    });

    it("does not treat a checkbox or other non-text input as editable", () => {
      // Chosen behavior: non-text inputs accept no typing, so paste-url may fire.
      expect(isEditableTarget({ tagName: "INPUT", type: "checkbox" })).toBe(false);
      expect(isEditableTarget({ tagName: "INPUT", type: "radio" })).toBe(false);
      expect(isEditableTarget({ tagName: "INPUT", type: "button" })).toBe(false);
      expect(isEditableTarget({ tagName: "INPUT", type: "FILE" })).toBe(false);
    });

    it("returns true for a contentEditable element", () => {
      expect(isEditableTarget({ tagName: "DIV", isContentEditable: true })).toBe(true);
      expect(isEditableTarget({ tagName: "SPAN", isContentEditable: true })).toBe(true);
    });

    it("returns false for a plain div, the body, and non-elements", () => {
      expect(isEditableTarget({ tagName: "DIV" })).toBe(false);
      expect(isEditableTarget({ tagName: "DIV", isContentEditable: false })).toBe(false);
      expect(isEditableTarget({ tagName: "BODY" })).toBe(false);
      expect(isEditableTarget(null)).toBe(false);
      expect(isEditableTarget(undefined)).toBe(false);
      expect(isEditableTarget({})).toBe(false);
    });
  });

  describe("matchPasteShortcut", () => {
    it("matches Ctrl+V, Cmd+V, and Shift+Insert", () => {
      expect(matchPasteShortcut({ key: "v", ctrlKey: true })).toBe("paste-url");
      expect(matchPasteShortcut({ key: "v", metaKey: true })).toBe("paste-url");
      expect(matchPasteShortcut({ key: "Insert", shiftKey: true })).toBe("paste-url");
      expect(matchPasteShortcut({ key: "insert", shiftKey: true })).toBe("paste-url");
    });

    it("is case-insensitive on the key", () => {
      expect(matchPasteShortcut({ key: "V", ctrlKey: true })).toBe("paste-url");
      expect(matchPasteShortcut({ key: "INSERT", shiftKey: true })).toBe("paste-url");
    });

    it("does not match without a modifier", () => {
      expect(matchPasteShortcut({ key: "v" })).toBeNull();
      expect(matchPasteShortcut({ key: "Insert" })).toBeNull();
    });

    it("does not match when Alt is held", () => {
      expect(matchPasteShortcut({ key: "v", ctrlKey: true, altKey: true })).toBeNull();
      expect(
        matchPasteShortcut({ key: "Insert", shiftKey: true, altKey: true })
      ).toBeNull();
    });

    it("does not match unrelated shortcuts", () => {
      expect(matchPasteShortcut({ key: "c", ctrlKey: true })).toBeNull();
      expect(matchPasteShortcut({ key: "n", ctrlKey: true })).toBeNull();
      expect(matchPasteShortcut({ key: "a", ctrlKey: true })).toBeNull();
      expect(matchPasteShortcut({ key: "Delete" })).toBeNull();
    });

    it("does not throw on partial or non-object events", () => {
      expect(matchPasteShortcut({})).toBeNull();
      expect(matchPasteShortcut({ key: undefined, ctrlKey: true })).toBeNull();
      expect(matchPasteShortcut(null)).toBeNull();
      expect(matchPasteShortcut(undefined)).toBeNull();
      expect(matchPasteShortcut("v")).toBeNull();
    });
  });

  describe("shouldHandleClipboardPaste", () => {
    const pasteEvent = { key: "v", ctrlKey: true, altKey: false, shiftKey: false };

    it("returns false when focus is inside an input, even on Ctrl+V (native paste wins)", () => {
      expect(
        shouldHandleClipboardPaste(pasteEvent, { tagName: "INPUT", type: "text" })
      ).toBe(false);
      expect(shouldHandleClipboardPaste(pasteEvent, { tagName: "TEXTAREA" })).toBe(false);
      expect(
        shouldHandleClipboardPaste(pasteEvent, { tagName: "DIV", isContentEditable: true })
      ).toBe(false);
      expect(
        shouldHandleClipboardPaste({ key: "Insert", shiftKey: true }, { tagName: "TEXTAREA" })
      ).toBe(false);
    });

    it("returns true on the document body with Ctrl+V", () => {
      expect(shouldHandleClipboardPaste(pasteEvent, { tagName: "BODY" })).toBe(true);
    });

    it("returns true with Shift+Insert on a div", () => {
      expect(
        shouldHandleClipboardPaste({ key: "Insert", shiftKey: true }, { tagName: "DIV" })
      ).toBe(true);
    });

    it("returns false when the event is not a paste shortcut, even outside inputs", () => {
      expect(shouldHandleClipboardPaste({ key: "c", ctrlKey: true }, { tagName: "BODY" })).toBe(
        false
      );
      expect(shouldHandleClipboardPaste({ key: "v" }, { tagName: "BODY" })).toBe(false);
      expect(shouldHandleClipboardPaste(null, { tagName: "BODY" })).toBe(false);
    });
  });

  describe("splitClipboardIntoActions", () => {
    it("returns none for an empty array or missing URLs", () => {
      expect(splitClipboardIntoActions([])).toEqual({ kind: "none" });
      expect(splitClipboardIntoActions(["", "   "])).toEqual({ kind: "none" });
    });

    it("returns none for non-array input", () => {
      expect(splitClipboardIntoActions(null)).toEqual({ kind: "none" });
      expect(splitClipboardIntoActions(undefined)).toEqual({ kind: "none" });
      expect(splitClipboardIntoActions("https://example.com/a.zip")).toEqual({ kind: "none" });
      expect(splitClipboardIntoActions({ length: 1, 0: "https://example.com/a.zip" })).toEqual({
        kind: "none",
      });
    });

    it("returns single for exactly one URL", () => {
      expect(splitClipboardIntoActions(["https://example.com/a.zip"])).toEqual({
        kind: "single",
        url: "https://example.com/a.zip",
      });
    });

    it("returns batch for multiple URLs, preserving order", () => {
      const urls = ["https://example.com/a.zip", "https://example.com/b.zip"];
      expect(splitClipboardIntoActions(urls)).toEqual({ kind: "batch", urls });
    });
  });
});

/**
 * Pure clipboard-paste logic for SAYL.
 *
 * Dependency-free on purpose: no DOM globals, no Tauri `invoke`, no i18n imports.
 * The only global used is `URL`, which exists in the webview and in Node (vitest).
 * `App.svelte` owns the side effects (toasts, dialogs, `invoke`); this module only
 * decides *what* the clipboard content means and *whether* paste should be handled.
 */

/** Maximum number of URLs accepted from one clipboard payload. */
export const MAX_CLIPBOARD_URLS = 100;

/** Shortcut id returned by `matchPasteShortcut` when the event is a paste-url. */
export const PASTE_URL_ACTION = "paste-url";

/** Schemes we accept as download links. `parsed.protocol` is always lowercase. */
const ALLOWED_SCHEMES = new Set(["http:", "https:", "ftp:"]);

/**
 * `<input>` types that do not accept typed text, so they are not "editing"
 * targets for the purposes of intercepting a paste shortcut.
 */
const NON_TEXT_INPUT_TYPES = new Set([
  "button",
  "checkbox",
  "color",
  "file",
  "hidden",
  "image",
  "radio",
  "range",
  "reset",
  "submit",
]);

/**
 * Parses a clipboard string into a de-duplicated list of valid download URLs.
 *
 * Every whitespace-separated token is a candidate, so both "one URL per line"
 * and space/tab separated lists work. Tokens are trimmed, must parse via the
 * `URL` global, must use `http:`, `https:` or `ftp:`, and must have a host.
 * De-duplication preserves first-seen order and the result is capped at
 * {@link MAX_CLIPBOARD_URLS} entries.
 *
 * @param {unknown} text Raw clipboard text (untrusted).
 * @returns {string[]} Valid URLs in first-seen order, or `[]` for non-strings.
 */
export function extractUrls(text) {
  if (typeof text !== "string" || text.length === 0) {
    return [];
  }

  const urls = [];
  const seen = new Set();

  for (const rawToken of text.split(/\s+/)) {
    if (urls.length >= MAX_CLIPBOARD_URLS) {
      break;
    }

    const token = rawToken.trim();
    if (!token) {
      continue;
    }

    let parsed;
    try {
      parsed = new URL(token);
    } catch {
      // Not an absolute URL, or unparseable garbage.
      continue;
    }

    if (!ALLOWED_SCHEMES.has(parsed.protocol)) {
      continue;
    }

    if (!parsed.hostname) {
      continue;
    }

    // Normalized href catches trivial variants ("https://a.com" vs "https://a.com/").
    const key = parsed.href;
    if (seen.has(key)) {
      continue;
    }

    seen.add(key);
    urls.push(token);
  }

  return urls;
}

/**
 * True when the active element accepts text input, i.e. a native paste must
 * keep working there instead of being hijacked by the global paste-url handler.
 *
 * Duck-typed (no `instanceof`/`document`), so it is safe to call with any object
 * and in non-DOM test environments.
 *
 * @param {unknown} el Usually `document.activeElement`.
 * @returns {boolean}
 */
export function isEditableTarget(el) {
  if (!el || typeof el !== "object") {
    return false;
  }

  const tag = typeof el.tagName === "string" ? el.tagName.toUpperCase() : "";

  if (tag === "INPUT") {
    const type = typeof el.type === "string" ? el.type.toLowerCase() : "";
    // Missing/unknown type defaults to "text", which is editable.
    return !NON_TEXT_INPUT_TYPES.has(type);
  }

  if (tag === "TEXTAREA" || tag === "SELECT") {
    return true;
  }

  if (el.isContentEditable === true) {
    return true;
  }

  return false;
}

/**
 * Classifies a keyboard event as the "paste a download URL" shortcut.
 *
 * Matches Ctrl+V, Cmd+V and Shift+Insert. Case-insensitive on the key, and never
 * matches while Alt is held (AltGr / dead-key compositions must not be stolen).
 * Tolerates partial or malformed event objects.
 *
 * @param {unknown} event
 * @returns {"paste-url"|null}
 */
export function matchPasteShortcut(event) {
  if (!event || typeof event !== "object") {
    return null;
  }

  if (event.altKey) {
    return null;
  }

  const key = typeof event.key === "string" ? event.key.toLowerCase() : "";
  if (!key) {
    return null;
  }

  if ((event.ctrlKey || event.metaKey) && key === "v") {
    return PASTE_URL_ACTION;
  }

  if (event.shiftKey && key === "insert") {
    return PASTE_URL_ACTION;
  }

  return null;
}

/**
 * Single focus-aware contract used by the App's global keydown handler:
 * intercept the paste shortcut only when focus is NOT inside an editable field.
 *
 * @param {unknown} event Keyboard event.
 * @param {unknown} activeElement Usually `document.activeElement`.
 * @returns {boolean} True when the app should handle the paste itself.
 */
export function shouldHandleClipboardPaste(event, activeElement) {
  if (matchPasteShortcut(event) !== PASTE_URL_ACTION) {
    return false;
  }

  return isEditableTarget(activeElement) === false;
}

/**
 * Maps the URL array returned by `get_clipboard_urls` onto a UI action.
 *
 * @param {unknown} urls
 * @returns {{ kind: "none" } | { kind: "single", url: string } | { kind: "batch", urls: string[] }}
 */
export function splitClipboardIntoActions(urls) {
  if (!Array.isArray(urls)) {
    return { kind: "none" };
  }

  const valid = urls.filter((url) => typeof url === "string" && url.trim() !== "");

  if (valid.length === 0) {
    return { kind: "none" };
  }

  if (valid.length === 1) {
    return { kind: "single", url: valid[0] };
  }

  return { kind: "batch", urls: valid };
}

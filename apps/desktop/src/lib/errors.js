/**
 * Friendly error classification module for SAYL download manager.
 * Maps backend error_kind to localized human-readable explanations and hints,
 * with graceful fallback for unknown errors.
 */

import { resolveTranslation } from "./i18n.js";

export const SUPPORTED_ERROR_KINDS = [
  "rate_limited",
  "service_unavailable",
  "range_unsupported",
  "connection_limit",
  "network",
  "not_found",
  "forbidden",
  "unauthorized",
  "disk",
  "integrity",
  "canceled",
  "http_429",
  "http_503",
  "http_5xx",
  "http_4xx",
  "too_many_redirects",
  "timeout",
  "invalid_url",
  "cancelled",
  "unknown",
];

/** Maps backend error_kind strings to the canonical translation key. */
const KIND_ALIASES = {
  http_429: "rate_limited",
  http_503: "service_unavailable",
  http_5xx: "service_unavailable",
  http_4xx: "forbidden",
  cancelled: "canceled",
};

/**
 * Returns a friendly { title, hint } object for a task's error.
 *
 * @param {{ error_kind?: string, error?: string } | null | undefined} task
 * @param {string} [lang="ar"]
 * @returns {{ title: string, hint: string }}
 */
export function friendlyError(task, lang = "ar") {
  const rawKind = task?.error_kind;
  const kind =
    typeof rawKind === "string" && SUPPORTED_ERROR_KINDS.includes(rawKind)
      ? rawKind
      : "unknown";

  const canonical = KIND_ALIASES[kind] || kind;
  const title = resolveTranslation(lang, `errors.${canonical}.title`);
  const hint = resolveTranslation(lang, `errors.${canonical}.hint`);

  return { title, hint };
}

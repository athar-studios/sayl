import { describe, it, expect } from "vitest";
import { friendlyError, SUPPORTED_ERROR_KINDS } from "./errors.js";

describe("Friendly errors module (errors.js)", () => {
  it("supports all 12 documented error kinds", () => {
    expect(SUPPORTED_ERROR_KINDS).toEqual([
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
      "unknown",
    ]);
  });

  it("maps each supported kind to localized title and hint in Arabic", () => {
    for (const kind of SUPPORTED_ERROR_KINDS) {
      const res = friendlyError({ error_kind: kind }, "ar");
      expect(res.title).toBeTruthy();
      expect(res.hint).toBeTruthy();
      expect(res.title).not.toContain("errors.");
      expect(res.hint).not.toContain("errors.");
    }
  });

  it("maps each supported kind to localized title and hint in English", () => {
    for (const kind of SUPPORTED_ERROR_KINDS) {
      const res = friendlyError({ error_kind: kind }, "en");
      expect(res.title).toBeTruthy();
      expect(res.hint).toBeTruthy();
      expect(res.title).not.toContain("errors.");
      expect(res.hint).not.toContain("errors.");
    }
  });

  it("matches the exact example hint for connection_limit in Arabic", () => {
    const res = friendlyError({ error_kind: "connection_limit" }, "ar");
    expect(res.hint).toBe("السيرفر رفض الاتصالات الكتير، سيل هيقلّلها ويحاول تاني");
  });

  it("falls back to 'unknown' error for unrecognized error kinds", () => {
    const unknownRes = friendlyError({ error_kind: "some_weird_unrecognized_error" }, "ar");
    const explicitUnknown = friendlyError({ error_kind: "unknown" }, "ar");
    expect(unknownRes.title).toBe(explicitUnknown.title);
    expect(unknownRes.hint).toBe(explicitUnknown.hint);
  });

  it("handles null and undefined task objects safely", () => {
    const nullRes = friendlyError(null, "ar");
    const undefRes = friendlyError(undefined, "ar");
    const emptyRes = friendlyError({}, "ar");

    const expected = friendlyError({ error_kind: "unknown" }, "ar");
    expect(nullRes).toEqual(expected);
    expect(undefRes).toEqual(expected);
    expect(emptyRes).toEqual(expected);
  });
});

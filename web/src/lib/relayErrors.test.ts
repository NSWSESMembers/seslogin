import { describe, it, expect } from "vitest";
import {
  getServerErrorMessage,
  getErrorCode,
  getErrorMessage,
} from "./relayErrors";

function relayNetworkError(
  errors: ReadonlyArray<{
    message?: string | null;
    extensions?: { code?: unknown } | null;
  }>,
) {
  const err = new Error("network error");
  (err as unknown as { source: { errors: typeof errors } }).source = {
    errors,
  };
  return err;
}

describe("getServerErrorMessage", () => {
  it("joins multiple GraphQL error messages", () => {
    const err = relayNetworkError([
      { message: "first problem" },
      { message: "second problem" },
    ]);
    expect(getServerErrorMessage(err)).toBe("first problem; second problem");
  });

  it("returns null for a plain error with no GraphQL response attached", () => {
    expect(getServerErrorMessage(new Error("fetch failed"))).toBeNull();
  });

  it("returns null for null/undefined", () => {
    expect(getServerErrorMessage(null)).toBeNull();
    expect(getServerErrorMessage(undefined)).toBeNull();
  });
});

describe("getErrorCode", () => {
  it("extracts extensions.code from the first classified error", () => {
    const err = relayNetworkError([
      {
        message: "Not authorized for this location",
        extensions: { code: "FORBIDDEN" },
      },
    ]);
    expect(getErrorCode(err)).toBe("FORBIDDEN");
  });

  it("skips an error with no code to find one on a later error", () => {
    const err = relayNetworkError([
      { message: "no code here" },
      { message: "this one is classified", extensions: { code: "NOT_FOUND" } },
    ]);
    expect(getErrorCode(err)).toBe("NOT_FOUND");
  });

  it("returns null when no error carries a code", () => {
    const err = relayNetworkError([{ message: "unclassified" }]);
    expect(getErrorCode(err)).toBeNull();
  });

  it("returns null for a non-GraphQL error", () => {
    expect(getErrorCode(new Error("network error"))).toBeNull();
  });
});

describe("getErrorMessage", () => {
  it("prefers friendly copy for UNAUTHENTICATED over the raw server message", () => {
    const err = relayNetworkError([
      {
        message: "Must provide user token",
        extensions: { code: "UNAUTHENTICATED" },
      },
    ]);
    expect(getErrorMessage(err)).toBe("You need to sign in again to do this.");
  });

  it("prefers friendly copy for FORBIDDEN over the raw server message", () => {
    const err = relayNetworkError([
      {
        message: "Not authorized for this location",
        extensions: { code: "FORBIDDEN" },
      },
    ]);
    expect(getErrorMessage(err)).toBe("You don't have access to do this.");
  });

  it("falls back to the server message for an unmapped code", () => {
    const err = relayNetworkError([
      {
        message: "Person with ID abc123 missing",
        extensions: { code: "NOT_FOUND" },
      },
    ]);
    expect(getErrorMessage(err)).toBe("Person with ID abc123 missing");
  });

  it("falls back to the Error's own message when there's no GraphQL response", () => {
    expect(getErrorMessage(new Error("boom"))).toBe("boom");
  });

  it("returns a fallback string for null/undefined", () => {
    expect(getErrorMessage(null)).toBe("Unknown error");
  });
});

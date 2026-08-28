import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  relayFieldLogger,
  takeRecentFieldErrorMessages,
  describeCaughtError,
} from "./relayFieldLogger";

function fieldPayloadError(message: string) {
  return {
    kind: "relay_field_payload.error" as const,
    owner: "SomeQuery",
    fieldPath: "location.periodSummaryByCategory.0.category",
    error: { message, path: [], severity: "ERROR" as const },
    shouldThrow: true,
    handled: false,
  };
}

function missingExpectedData() {
  return {
    kind: "missing_expected_data.throw" as const,
    owner: "SomeQuery",
    fieldPath: "location.periodSummaryByMember",
    handled: false,
  };
}

describe("relayFieldLogger message recovery", () => {
  beforeEach(() => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    vi.spyOn(console, "warn").mockImplementation(() => {});
    // Drain any leftovers from a prior test.
    takeRecentFieldErrorMessages();
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("buffers the real message from a relay_field_payload.error event", () => {
    relayFieldLogger(fieldPayloadError("Category with ID abc123 missing"));
    expect(takeRecentFieldErrorMessages()).toEqual([
      "Category with ID abc123 missing",
    ]);
  });

  it("does not buffer anything for a missing_expected_data event", () => {
    relayFieldLogger(missingExpectedData());
    expect(takeRecentFieldErrorMessages()).toEqual([]);
  });

  it("clears the buffer on read", () => {
    relayFieldLogger(fieldPayloadError("first"));
    takeRecentFieldErrorMessages();
    expect(takeRecentFieldErrorMessages()).toEqual([]);
  });

  it("caps the buffer so it can't grow unboundedly", () => {
    for (let i = 0; i < 10; i++) {
      relayFieldLogger(fieldPayloadError(`error ${i}`));
    }
    const messages = takeRecentFieldErrorMessages();
    expect(messages.length).toBeLessThanOrEqual(5);
    // Keeps the most recent ones, not the oldest.
    expect(messages[messages.length - 1]).toBe("error 9");
  });
});

describe("describeCaughtError", () => {
  beforeEach(() => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    vi.spyOn(console, "warn").mockImplementation(() => {});
    takeRecentFieldErrorMessages();
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("substitutes the real message for Relay's generic 'missing expected data' text", () => {
    // The scenario this exists for: a real field error (real message, logged) fires
    // in the same batch as a generic missing-data throw for a field earlier in the
    // query's selection order — the one that actually reaches the component.
    relayFieldLogger(fieldPayloadError("Category with ID abc123 missing"));
    const thrown = new Error(
      "Relay: Missing expected data at path 'location.periodSummaryByMember' in 'ActivityTotalsDisplayQuery'.",
    );
    expect(describeCaughtError(thrown)).toBe("Category with ID abc123 missing");
  });

  it("substitutes the real message for Relay's generic field-payload text", () => {
    relayFieldLogger(fieldPayloadError("Category with ID abc123 missing"));
    const thrown = new Error(
      "Relay: Unexpected response payload - check server logs for details.",
    );
    expect(describeCaughtError(thrown)).toBe("Category with ID abc123 missing");
  });

  it("falls back to the generic message when nothing was buffered", () => {
    const thrown = new Error(
      "Relay: Missing expected data at path 'x' in 'y'.",
    );
    expect(describeCaughtError(thrown)).toBe(thrown.message);
  });

  it("leaves a non-Relay error's message untouched even if something was buffered", () => {
    relayFieldLogger(fieldPayloadError("unrelated buffered message"));
    const thrown = new Error(
      "could not sign out Jane: start must be before end",
    );
    expect(describeCaughtError(thrown)).toBe(
      "could not sign out Jane: start must be before end",
    );
  });

  it("leaves a whole-query 'No data returned' error untouched, since it already carries the real text", () => {
    const thrown = new Error(
      "No data returned for operation `ActivityTotalsDisplayQuery`, got error(s):\nCategory with ID abc123 missing",
    );
    expect(describeCaughtError(thrown)).toBe(thrown.message);
  });

  it("stringifies a non-Error value", () => {
    expect(describeCaughtError("just a string")).toBe("just a string");
  });
});

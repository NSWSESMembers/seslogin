import { describe, it, expect } from "vitest";
import { formatFullDateTime } from "../../lib/time";
import {
  actorText,
  entityTypeLabel,
  formatChangeValue,
} from "./auditLogFormat";

describe("formatChangeValue", () => {
  it("shows a dash for an absent value", () => {
    expect(formatChangeValue("first_name", null)).toBe("—");
  });

  it("formats epoch seconds in time fields as dates", () => {
    const expected = formatFullDateTime(new Date(1_700_000_000 * 1000));
    expect(formatChangeValue("start_time", "1700000000")).toBe(expected);
    expect(formatChangeValue("expires_at", "1700000000")).toBe(expected);
    expect(formatChangeValue("deleted", "1700000000")).toBe(expected);
    expect(formatChangeValue("missing_since", "1700000000")).toBe(expected);
  });

  it("leaves numbers in other fields and non-numeric time values alone", () => {
    expect(formatChangeValue("member_number", "1700000000")).toBe("1700000000");
    expect(formatChangeValue("end_time", "soon")).toBe("soon");
    expect(formatChangeValue("deleted", "true")).toBe("true");
  });
});

describe("actorText", () => {
  it("prefers the resolved label", () => {
    expect(actorText({ kind: "USER", id: "u1", label: "a@b.test" })).toBe(
      "a@b.test",
    );
  });

  it("falls back to kind-specific text", () => {
    expect(actorText({ kind: "SESSION", id: "s", label: null })).toBe("Kiosk");
    expect(actorText({ kind: "SYSTEM", id: "member-sync", label: null })).toBe(
      "System: member-sync",
    );
    expect(actorText({ kind: "UNKNOWN", id: null, label: null })).toBe(
      "Unknown",
    );
  });
});

describe("entityTypeLabel", () => {
  it("uses the app's vocabulary", () => {
    expect(entityTypeLabel("PERSON")).toBe("Member");
    expect(entityTypeLabel("PERIOD")).toBe("Activity");
    expect(entityTypeLabel("SESSION")).toBe("Kiosk");
  });
});

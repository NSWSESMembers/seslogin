import { describe, expect, it } from "vitest";
import { describeStatus, kioskStatus } from "./kioskStatus";

const now = 1_800_000_000;
const kiosk = (overrides: Partial<Parameters<typeof kioskStatus>[0]> = {}) => ({
  id: "k1",
  keyEnrolled: true,
  keyReleasedAt: null,
  lastContact: now - 60,
  ...overrides,
});

describe("kioskStatus", () => {
  it("names the computer's own kiosk, whatever else is true of it", () => {
    expect(kioskStatus(kiosk({ lastContact: null }), "k1", now)).toEqual({
      kind: "this",
    });
  });

  it("says a kiosk with no key, or a released one, has no computer", () => {
    expect(kioskStatus(kiosk({ keyEnrolled: false }), null, now).kind).toBe(
      "none",
    );
    expect(kioskStatus(kiosk({ keyReleasedAt: now - 5 }), null, now).kind).toBe(
      "none",
    );
  });

  it("says a kiosk seen in the last 20 minutes is online elsewhere", () => {
    expect(kioskStatus(kiosk(), null, now).kind).toBe("online");
    expect(
      kioskStatus(kiosk({ lastContact: now - 20 * 60 }), null, now),
    ).toEqual({ kind: "offline", since: now - 20 * 60 });
  });

  it("describes a kiosk never seen", () => {
    expect(
      describeStatus(kioskStatus(kiosk({ lastContact: null }), null, now)),
    ).toBe("never seen");
  });
});

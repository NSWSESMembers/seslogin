import { describe, expect, it } from "vitest";
import type { KioskSession } from "../components/KioskSessionContext";
import {
  acceptFetchedSession,
  applySessionUpdated,
  parseSessionChanged,
  parseSessionUpdated,
} from "./sessionPush";

const session: KioskSession = {
  id: "s1",
  name: "Front desk",
  config: { theme: "light" },
  version: 3,
  location: { id: "loc1", name: "Unit A" },
};

describe("parseSessionUpdated", () => {
  it("accepts a well-formed payload", () => {
    expect(
      parseSessionUpdated({
        sessionId: "s1",
        version: 4,
        name: "Back door",
        config: { theme: "dark" },
      }),
    ).toEqual({
      sessionId: "s1",
      version: 4,
      name: "Back door",
      config: { theme: "dark" },
    });
  });

  it.each([
    null,
    "nope",
    { sessionId: "s1", version: "4", name: "x", config: {} },
    { sessionId: "s1", version: 4, name: "x", config: null },
    { sessionId: "s1", version: 4, name: "x", config: [] },
    { version: 4, name: "x", config: {} },
  ])("rejects %j", (data) => {
    expect(parseSessionUpdated(data)).toBeNull();
  });
});

describe("parseSessionChanged", () => {
  it("accepts a well-formed payload and rejects junk", () => {
    expect(parseSessionChanged({ sessionId: "s1", version: 2 })).toEqual({
      sessionId: "s1",
      version: 2,
    });
    expect(parseSessionChanged({ sessionId: "s1" })).toBeNull();
    expect(parseSessionChanged(undefined)).toBeNull();
  });
});

describe("applySessionUpdated", () => {
  const push = {
    sessionId: "s1",
    name: "Back door",
    config: { theme: "dark" },
  };

  it("applies a newer push for this session, keeping the location", () => {
    expect(applySessionUpdated(session, { ...push, version: 4 })).toEqual({
      ...session,
      name: "Back door",
      config: { theme: "dark" },
      version: 4,
    });
  });

  it("ignores a push at the same or an older version", () => {
    expect(applySessionUpdated(session, { ...push, version: 3 })).toBe(session);
    expect(applySessionUpdated(session, { ...push, version: 2 })).toBe(session);
  });

  it("ignores a push for another session, or with no session yet", () => {
    expect(
      applySessionUpdated(session, { ...push, sessionId: "s2", version: 9 }),
    ).toBe(session);
    expect(applySessionUpdated(null, { ...push, version: 9 })).toBeNull();
  });
});

describe("acceptFetchedSession", () => {
  it("rejects a refetch of the same session older than what's held", () => {
    expect(acceptFetchedSession(session, { ...session, version: 2 })).toBe(
      false,
    );
  });

  it("accepts the same session at an equal or newer version", () => {
    expect(acceptFetchedSession(session, { ...session })).toBe(true);
    expect(acceptFetchedSession(session, { ...session, version: 5 })).toBe(
      true,
    );
  });

  it("always accepts a different session, or none", () => {
    expect(
      acceptFetchedSession(session, { ...session, id: "s2", version: 1 }),
    ).toBe(true);
    expect(acceptFetchedSession(session, null)).toBe(true);
    expect(acceptFetchedSession(null, session)).toBe(true);
  });
});

import { describe, it, expect } from "vitest";
import { kioskBadgesEnabled } from "./kioskBadges";
import type { KioskSession } from "../components/KioskSessionContext";

function session(config: KioskSession["config"]): KioskSession {
  return {
    id: "sess1",
    name: "Kiosk",
    config,
    version: 1,
    location: { id: "loc1", name: "Unit A" },
  };
}

describe("kioskBadgesEnabled", () => {
  it("follows the kiosk's own option", () => {
    expect(kioskBadgesEnabled(session({ gamification: true }))).toBe(true);
    expect(kioskBadgesEnabled(session({}))).toBe(false);
  });

  it("only counts a real true, as the server does", () => {
    expect(kioskBadgesEnabled(session({ gamification: "true" }))).toBe(false);
    expect(kioskBadgesEnabled(null)).toBe(false);
  });
});

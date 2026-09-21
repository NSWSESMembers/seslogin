import { describe, expect, it } from "vitest";
import {
  TIER_ICON_PALETTE,
  TIER_ORDER_DESC,
  TIER_PILL_CLASS,
  tierCssClass,
  tierKey,
  tierLabel,
} from "./badgeTiers";

describe("tierKey", () => {
  it("lowercases and trims a tier name", () => {
    expect(tierKey("  Gold  ")).toBe("gold");
  });

  it("leaves internal whitespace alone", () => {
    expect(tierKey("Passport Stamps")).toBe("passport stamps");
  });
});

describe("tierCssClass", () => {
  it("collapses whitespace into a single hyphen", () => {
    expect(tierCssClass("Passport Stamps")).toBe("passport-stamps");
  });

  it("matches TIER_PILL_CLASS's keys for every known tier", () => {
    for (const tier of TIER_ORDER_DESC) {
      expect(TIER_PILL_CLASS[tierCssClass(tier)]).toBeDefined();
    }
  });
});

describe("tierLabel", () => {
  it("title-cases a single-word tier and appends 'Tier'", () => {
    expect(tierLabel("gold")).toBe("Gold Tier");
  });

  it("title-cases every word of a multi-word tier", () => {
    expect(tierLabel("passport stamps")).toBe("Passport Stamps Tier");
  });

  it("ignores extra whitespace between words", () => {
    expect(tierLabel("passport   stamps")).toBe("Passport Stamps Tier");
  });
});

describe("TIER_ICON_PALETTE", () => {
  it("has an entry for every tier in TIER_ORDER_DESC", () => {
    for (const tier of TIER_ORDER_DESC) {
      expect(
        TIER_ICON_PALETTE[tierCssClass(tier) as keyof typeof TIER_ICON_PALETTE],
      ).toBeDefined();
    }
  });

  it("matches TIER_PILL_CLASS's background token for every tier", () => {
    // TIER_PILL_CLASS's first class is always `bg-tier-<tier>`, and
    // TIER_ICON_PALETTE.outer is meant to render the same color (see the
    // doc comment on TIER_ICON_PALETTE) — assert the token names line up so
    // the two can't silently drift apart again the way passport stamps once
    // did in the stale version of this file.
    for (const [tier, palette] of Object.entries(TIER_ICON_PALETTE)) {
      const pillClass = TIER_PILL_CLASS[tier];
      expect(pillClass).toContain(`bg-tier-${tier}`);
      expect(palette.outer).toMatch(/^#[0-9a-f]{6}$/);
    }
  });
});

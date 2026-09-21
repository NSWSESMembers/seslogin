export const TIER_ORDER_DESC = [
  "gold",
  "silver",
  "bronze",
  "starter",
  "passport stamps",
] as const;

export function tierKey(tier: string) {
  return tier.trim().toLowerCase();
}

export function tierCssClass(tier: string) {
  return tierKey(tier).replace(/\s+/g, "-");
}

export function tierLabel(tier: string) {
  return `${tier
    .split(/\s+/)
    .filter(Boolean)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ")} Tier`;
}

/** Background/text/border classes for the tier-colored pills and headings used
 * across the admin and kiosk badge UI. Backed by the `--color-tier-*` tokens
 * in app.css (light values there, dark values duplicated across both dark
 * blocks) rather than raw hex, since these colors are reused across files. */
export const TIER_PILL_CLASS: Record<string, string> = {
  gold: "bg-tier-gold text-tier-gold-ink border-tier-gold-border",
  silver: "bg-tier-silver text-tier-silver-ink border-tier-silver-border",
  bronze: "bg-tier-bronze text-tier-bronze-ink border-tier-bronze-border",
  starter: "bg-tier-starter text-tier-starter-ink border-tier-starter-border",
  "passport-stamps":
    "bg-tier-passport-stamps text-tier-passport-stamps-ink border-tier-passport-stamps-border",
};

export type BadgeTier =
  "starter" | "passport-stamps" | "bronze" | "silver" | "gold";

export type BadgeIconPalette = {
  outer: string;
  inner: string;
  ring: string;
  symbol: string;
};

/**
 * Per-tier fill/stroke palette for the badge icon artwork drawn in
 * `lib/badgeIcons.tsx` (`StaticBadgeIconSvg`'s gradient/ring/symbol). Kept
 * here as literal hex, not as CSS custom properties, because SVG
 * presentation attributes (`fill`/`stroke`) can only lean on the
 * `icon-stroke`/`icon-fill`/`icon-accent` `currentColor`+`--icon-accent`
 * convention (app.css) for a *two*-channel palette; this one needs four
 * independent colors per tier (outer, inner, ring, symbol), which that
 * convention doesn't cover. Living here rather than duplicated in
 * badgeIcons.tsx keeps there being exactly one place that defines what each
 * tier looks like. `outer` intentionally matches `TIER_PILL_CLASS`'s
 * background for every tier (the stale version of this file had passport
 * stamps' outer color drift slightly from its pill background — fixed here).
 */
export const TIER_ICON_PALETTE: Record<BadgeTier, BadgeIconPalette> = {
  "passport-stamps": {
    outer: "#e9f7ef",
    inner: "#72c39a",
    ring: "#2d7352",
    symbol: "#174732",
  },
  gold: {
    outer: "#fff4be",
    inner: "#f4be2e",
    ring: "#9b6f00",
    symbol: "#5f3c00",
  },
  silver: {
    outer: "#f2f6fb",
    inner: "#bac6d2",
    ring: "#5f6d7f",
    symbol: "#243141",
  },
  bronze: {
    outer: "#f6dfcf",
    inner: "#cb8b57",
    ring: "#7d4b21",
    symbol: "#3d1f08",
  },
  starter: {
    outer: "#f0f6ff",
    inner: "#9fbbe0",
    ring: "#3f5e86",
    symbol: "#173358",
  },
};

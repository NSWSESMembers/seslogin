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
 * across the admin and kiosk badge UI. */
export const TIER_PILL_CLASS: Record<string, string> = {
  gold: "bg-[#fff4be] text-[#7a4e00] border-[#b98a00]",
  silver: "bg-[#f2f6fb] text-[#3a4c5e] border-[#7b8794]",
  bronze: "bg-[#f6dfcf] text-[#5c2e0a] border-[#b06f3e]",
  starter: "bg-[#f0f6ff] text-[#1a3a62] border-[#6a90b8]",
  "passport-stamps": "bg-[#e9f7ef] text-[#1d5a3f] border-[#3f8f68]",
};

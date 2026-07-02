import { useId } from "react";

type BadgeTier = "starter" | "passport-stamps" | "bronze" | "silver" | "gold";

type BadgeIconProps = {
  badgeId: string;
  badgeName?: string;
  tier?: string;
  className?: string;
};

const FIRST_SIGNIN_LOCATION_BADGE_PREFIX = "first-signin-location-";
const FIRST_AWAY_SIGNIN_LOCATION_BADGE_PREFIX = "first-away-signin-location-";

type Palette = {
  outer: string;
  inner: string;
  ring: string;
  symbol: string;
};

function paletteForTier(tier: BadgeTier): Palette {
  if (tier === "passport-stamps") {
    return {
      outer: "#e6f7ee",
      inner: "#72c39a",
      ring: "#2d7352",
      symbol: "#174732",
    };
  }
  if (tier === "gold") {
    return {
      outer: "#fff4be",
      inner: "#f4be2e",
      ring: "#9b6f00",
      symbol: "#5f3c00",
    };
  }
  if (tier === "silver") {
    return {
      outer: "#f2f6fb",
      inner: "#bac6d2",
      ring: "#5f6d7f",
      symbol: "#243141",
    };
  }
  if (tier === "bronze") {
    return {
      outer: "#f6dfcf",
      inner: "#cb8b57",
      ring: "#7d4b21",
      symbol: "#3d1f08",
    };
  }
  return {
    outer: "#f0f6ff",
    inner: "#9fbbe0",
    ring: "#3f5e86",
    symbol: "#173358",
  };
}

function normalizeTier(tier: string | undefined): BadgeTier {
  if (
    tier === "passport-stamps" ||
    tier === "passport stamps" ||
    tier === "passport_stamps"
  ) {
    return "passport-stamps";
  }
  if (tier === "gold" || tier === "silver" || tier === "bronze") {
    return tier;
  }
  return "starter";
}

function isLocationStampBadgeId(badgeId: string): boolean {
  return (
    badgeId.startsWith(FIRST_SIGNIN_LOCATION_BADGE_PREFIX) ||
    badgeId.startsWith(FIRST_AWAY_SIGNIN_LOCATION_BADGE_PREFIX)
  );
}

function stampTextFromUnitName(unitName: string | undefined): string {
  if (!unitName) {
    return "UNIT";
  }

  const cleaned = unitName
    .replace(/[^A-Za-z0-9 ]+/g, " ")
    .trim()
    .replace(/\s+/g, " ");
  if (!cleaned) {
    return "UNIT";
  }

  const words = cleaned.split(" ").filter(Boolean);
  if (words.length >= 2) {
    const initials = words
      .slice(0, 3)
      .map((word) => word.charAt(0))
      .join("")
      .toUpperCase();
    if (initials.length >= 2) {
      return initials;
    }
  }

  return cleaned.toUpperCase().slice(0, 4);
}

function SymbolForBadge({
  badgeId,
  badgeName,
  color,
}: {
  badgeId: string;
  badgeName?: string;
  color: string;
}) {
  if (isLocationStampBadgeId(badgeId)) {
    const stampText = stampTextFromUnitName(badgeName);
    const fontSize = stampText.length >= 4 ? 9.5 : 11;
    return (
      <>
        <circle
          cx="38"
          cy="38"
          r="22"
          fill="none"
          stroke={color}
          strokeWidth="3.2"
          strokeDasharray="2.6 2.2"
        />
        <circle
          cx="38"
          cy="38"
          r="17"
          fill="none"
          stroke={color}
          strokeWidth="2"
        />
        <circle
          cx="38"
          cy="38"
          r="9"
          fill="none"
          stroke={color}
          strokeWidth="1.8"
        />
        <g transform="rotate(-25 38 38)">
          <line
            x1="18"
            y1="38"
            x2="58"
            y2="38"
            stroke={color}
            strokeWidth="2.2"
            strokeLinecap="round"
          />
          <text
            x="38"
            y="35.6"
            textAnchor="middle"
            fill={color}
            fontWeight="800"
            fontSize={fontSize}
            fontFamily="TitilliumMapsTitle, Verdana, sans-serif"
            letterSpacing="0.14em"
            paintOrder="stroke"
            stroke="#ffffff"
            strokeWidth="1.3"
          >
            {stampText}
          </text>
        </g>
      </>
    );
  }

  // ── General badges ─────────────────────────────────────────────────────────
  if (badgeId === "first-steps") {
    return (
      <>
        <path d="M34 44h8l2 16h-12z" fill={color} />
        <circle cx="38" cy="33" r="6" fill={color} />
      </>
    );
  }
  if (badgeId === "weekend-warmer") {
    return (
      <>
        <path d="M24 44l14-20 14 20-4 4H28z" fill={color} />
        <rect x="35" y="20" width="6" height="8" fill={color} />
      </>
    );
  }
  if (badgeId === "steady-strider") {
    return (
      <>
        <path
          d="M22 48l10-8 8 4 16-14"
          stroke={color}
          strokeWidth="5"
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <circle cx="22" cy="48" r="3" fill={color} />
        <circle cx="32" cy="40" r="3" fill={color} />
        <circle cx="40" cy="44" r="3" fill={color} />
        <circle cx="56" cy="30" r="3" fill={color} />
      </>
    );
  }
  if (badgeId === "legacy-legend") {
    return (
      <path
        d="M38 20l5 10 11 2-8 8 2 11-10-5-10 5 2-11-8-8 11-2z"
        fill={color}
      />
    );
  }
  if (badgeId === "clean-finish") {
    return (
      <path
        d="M24 38l9 9 19-19"
        stroke={color}
        strokeWidth="6"
        fill="none"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    );
  }
  if (badgeId === "accuracy-ace") {
    return (
      <>
        <circle
          cx="38"
          cy="38"
          r="14"
          stroke={color}
          strokeWidth="4"
          fill="none"
        />
        <circle
          cx="38"
          cy="38"
          r="7"
          stroke={color}
          strokeWidth="4"
          fill="none"
        />
        <circle cx="38" cy="38" r="2.5" fill={color} />
      </>
    );
  }
  if (badgeId === "four-week-rhythm") {
    return (
      <>
        <rect
          x="22"
          y="26"
          width="32"
          height="24"
          rx="4"
          fill="none"
          stroke={color}
          strokeWidth="4"
        />
        <rect x="28" y="18" width="4" height="8" fill={color} />
        <rect x="44" y="18" width="4" height="8" fill={color} />
        <path
          d="M28 38h20"
          stroke={color}
          strokeWidth="4"
          strokeLinecap="round"
        />
        <path
          d="M28 45h10"
          stroke={color}
          strokeWidth="4"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "away-checkin-suitcase-stowaway") {
    return (
      <>
        <rect
          x="23"
          y="28"
          width="30"
          height="22"
          rx="4"
          fill="none"
          stroke={color}
          strokeWidth="4"
        />
        <path
          d="M32 28v-5h12v5"
          stroke={color}
          strokeWidth="4"
          fill="none"
          strokeLinecap="round"
        />
        <path
          d="M38 34v10"
          stroke={color}
          strokeWidth="4"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "away-checkin-map-goblin") {
    return (
      <>
        <path
          d="M22 48V24l11-4 10 4 11-4v24l-11 4-10-4z"
          fill="none"
          stroke={color}
          strokeWidth="4"
          strokeLinejoin="round"
        />
        <path
          d="M33 20v24M43 24v24"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "away-checkin-road-comedian") {
    return (
      <>
        <rect
          x="24"
          y="32"
          width="28"
          height="12"
          rx="5"
          fill="none"
          stroke={color}
          strokeWidth="4"
        />
        <path
          d="M28 32l5-6h10l5 6"
          stroke={color}
          strokeWidth="4"
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <circle cx="30" cy="46" r="3" fill={color} />
        <circle cx="46" cy="46" r="3" fill={color} />
      </>
    );
  }
  if (badgeId === "away-checkin-interstate-icon") {
    return (
      <>
        <path
          d="M24 44c12 0 20-6 30-20"
          stroke={color}
          strokeWidth="4"
          fill="none"
          strokeLinecap="round"
        />
        <path
          d="M46 22h8v8"
          stroke={color}
          strokeWidth="4"
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <path
          d="M22 30h10"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
        <path
          d="M20 36h8"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
      </>
    );
  }

  // ── Combat Roles - Storm ────────────────────────────────────────────────────
  if (badgeId === "combat-roles-storm-eye-opener") {
    return <path d="M42 20l-9 16h8l-9 18 14-20h-8z" fill={color} />;
  }
  if (badgeId === "combat-roles-storm-thunder-ten") {
    return (
      <>
        <circle
          cx="38"
          cy="38"
          r="15"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path d="M41 25l-7 13h6l-6 14 12-18h-7z" fill={color} />
      </>
    );
  }
  if (badgeId === "combat-roles-storm-gale-force") {
    return (
      <>
        <path
          d="M26 32c6-8 18-8 18 0s-12 8-12 0"
          stroke={color}
          strokeWidth="4"
          fill="none"
          strokeLinecap="round"
        />
        <path
          d="M26 38c6-8 20-8 20 0"
          stroke={color}
          strokeWidth="4"
          fill="none"
          strokeLinecap="round"
        />
        <path
          d="M26 44c6-8 22-8 22 0"
          stroke={color}
          strokeWidth="4"
          fill="none"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "combat-roles-storm-calm-after") {
    return (
      <>
        <circle cx="38" cy="38" r="7" fill={color} />
        <path
          d="M38 22v4M38 50v4M22 38h4M50 38h4M27 27l3 3M45 45l3 3M27 49l3-3M45 31l3-3"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "combat-roles-storm-storm-trooper-streak") {
    return (
      <>
        <path
          d="M24 34c0-9 6-15 14-15s14 6 14 15v7c0 8-6 14-14 14s-14-6-14-14v-7z"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M22 34h32"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
        <circle cx="32" cy="39" r="3" fill={color} />
        <circle cx="44" cy="39" r="3" fill={color} />
        <path
          d="M30 48h16"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
        <path
          d="M28 24l4 3 6-4 6 4 4-3"
          stroke={color}
          strokeWidth="3"
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <path
          d="M20 56h36"
          stroke={color}
          strokeWidth="3"
          strokeLinecap="round"
          strokeDasharray="4 4"
        />
      </>
    );
  }

  // ── Trainer - Storm & Water ─────────────────────────────────────────────────
  if (badgeId === "trainer-storm-water-splash-class") {
    return (
      <>
        <path
          d="M24 33c0-9 6-15 14-15s14 6 14 15v7c0 8-6 14-14 14s-14-6-14-14v-7z"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M20 33h36"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
        <circle cx="32" cy="38" r="3" fill={color} />
        <circle cx="44" cy="38" r="3" fill={color} />
        <path
          d="M30 48h16"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
        <path
          d="M26 23c4-5 20-5 24 0"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "trainer-storm-water-brain-rain") {
    return (
      <>
        <path
          d="M26 22c0 0-5 7-5 11 0 3 2 5 5 5s5-2 5-5c0-4-5-11-5-11z"
          fill={color}
        />
        <path
          d="M38 16c0 0-5 8-5 12 0 3 2 5 5 5s5-2 5-5c0-4-5-12-5-12z"
          fill={color}
        />
        <path
          d="M50 22c0 0-5 7-5 11 0 3 2 5 5 5s5-2 5-5c0-4-5-11-5-11z"
          fill={color}
        />
        <path
          d="M18 52h40"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
        <path
          d="M22 46h32"
          stroke={color}
          strokeWidth="2"
          strokeLinecap="round"
          strokeDasharray="4 4"
        />
      </>
    );
  }
  if (badgeId === "trainer-storm-water-current-curriculum") {
    return (
      <>
        <path
          d="M18 32c4 0 4 8 8 8s4-8 8-8 4 8 8 8 4-8 8-8"
          stroke={color}
          strokeWidth="4.5"
          fill="none"
          strokeLinecap="round"
        />
        <path
          d="M18 44c4 0 4 8 8 8s4-8 8-8 4 8 8 8 4-8 8-8"
          stroke={color}
          strokeWidth="4.5"
          fill="none"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "trainer-storm-water-mastercaster") {
    return (
      <>
        <circle
          cx="38"
          cy="27"
          r="5"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M38 32v22"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
        <path
          d="M28 41c0 0 2 14 10 14s10-14 10-14"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinecap="round"
        />
        <path
          d="M30 32h16"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
      </>
    );
  }

  // ── Training - Beacon ───────────────────────────────────────────────────────
  if (badgeId === "training-beacon-light-work") {
    return (
      <>
        <path
          d="M38 44v12"
          stroke={color}
          strokeWidth="4"
          strokeLinecap="round"
        />
        <circle cx="38" cy="42" r="3" fill={color} />
        <path
          d="M30 34c0 0 2-8 8-8s8 8 8 8"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinecap="round"
        />
        <path
          d="M22 26c0 0 5-14 16-14s16 14 16 14"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "training-beacon-bright-idea") {
    return (
      <>
        <path
          d="M38 23c-7 0-12 5-12 12 0 5 3 9 7 11v3h10v-3c4-2 7-6 7-11 0-7-5-12-12-12z"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M33 49h10"
          stroke={color}
          strokeWidth="3"
          strokeLinecap="round"
        />
        <path
          d="M34 53h8"
          stroke={color}
          strokeWidth="3"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "training-beacon-guiding-light") {
    return (
      <>
        <circle cx="38" cy="34" r="8" fill={color} />
        <path
          d="M38 18v4M38 46v4M22 34h4M48 34h4M26 22l3 3M47 45l3 3M26 46l3-3M47 23l3-3"
          stroke={color}
          strokeWidth="3"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "training-beacon-lighthouse-legend") {
    return (
      <>
        <rect x="33" y="30" width="10" height="26" rx="1" fill={color} />
        <path d="M30 30h16l-2-10h-12z" fill={color} />
        <path
          d="M26 22c-4-2-6-6-6-6"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinecap="round"
        />
        <path
          d="M50 22c4-2 6-6 6-6"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinecap="round"
        />
        <rect x="30" y="54" width="16" height="4" rx="2" fill={color} />
      </>
    );
  }

  // ── Training - Job Ready ────────────────────────────────────────────────────
  if (badgeId === "training-job-ready-hired-vibe") {
    return (
      <>
        <rect
          x="22"
          y="32"
          width="32"
          height="24"
          rx="4"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M32 32v-4h12v4"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinecap="round"
        />
        <path
          d="M22 42h32"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "training-job-ready-role-model") {
    return (
      <>
        <circle cx="38" cy="27" r="7" fill={color} />
        <path d="M22 56c0-9 7-16 16-16s16 7 16 16" fill={color} />
      </>
    );
  }
  if (badgeId === "training-job-ready-ready-set-go") {
    return (
      <>
        <path
          d="M22 30h5M22 38h5M22 46h5"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
        <path
          d="M32 38h20"
          stroke={color}
          strokeWidth="4"
          strokeLinecap="round"
        />
        <path
          d="M46 29l10 9-10 9"
          stroke={color}
          strokeWidth="4"
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </>
    );
  }
  if (badgeId === "training-job-ready-job-juggler") {
    return (
      <>
        <path
          d="M28 22h20v14c0 8-4 12-10 12s-10-4-10-12V22z"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M28 28c-5 0-7 5-4 9"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinecap="round"
        />
        <path
          d="M48 28c5 0 7 5 4 9"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinecap="round"
        />
        <path
          d="M34 48v6"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
        <path
          d="M42 48v6"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
        <path
          d="M30 54h16"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
      </>
    );
  }

  // ── Training - Fit for Role ─────────────────────────────────────────────────
  if (badgeId === "training-fit-for-role-perfect-fit") {
    return (
      <>
        <circle
          cx="38"
          cy="38"
          r="14"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <circle
          cx="38"
          cy="38"
          r="7"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <circle cx="38" cy="38" r="2.5" fill={color} />
      </>
    );
  }
  if (badgeId === "training-fit-for-role-fit-happens") {
    return (
      <path
        d="M38 22l14 6v10c0 10-6 16-14 18-8-2-14-8-14-18V28z"
        stroke={color}
        strokeWidth="3.5"
        fill="none"
        strokeLinejoin="round"
      />
    );
  }
  if (badgeId === "training-fit-for-role-suited-up") {
    return (
      <>
        <path
          d="M38 22l14 6v10c0 10-6 16-14 18-8-2-14-8-14-18V28z"
          fill={color}
          strokeLinejoin="round"
        />
        <path
          d="M28 38l8 8 12-14"
          stroke="rgba(255,255,255,0.85)"
          strokeWidth="4.5"
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </>
    );
  }
  if (badgeId === "training-fit-for-role-tailor-made") {
    return <path d="M38 20l16 18-16 18-16-18z" fill={color} />;
  }

  // ── Other - Other ───────────────────────────────────────────────────────────
  if (badgeId === "other-other-oddball") {
    return (
      <>
        <path
          d="M30 28c0-5 4-9 8-9s8 4 8 9c0 4-4 6-8 10"
          stroke={color}
          strokeWidth="4.5"
          fill="none"
          strokeLinecap="round"
        />
        <circle cx="38" cy="52" r="3.5" fill={color} />
      </>
    );
  }
  if (badgeId === "other-other-misc-chief") {
    return (
      <>
        <circle cx="22" cy="38" r="5" fill={color} />
        <circle cx="38" cy="38" r="5" fill={color} />
        <circle cx="54" cy="38" r="5" fill={color} />
      </>
    );
  }
  if (badgeId === "other-other-etcetera-veteran") {
    return (
      <path
        d="M18 38 C18 26 34 26 38 38 C42 50 58 50 58 38 C58 26 42 26 38 38 C34 50 18 50 18 38 Z"
        stroke={color}
        strokeWidth="4"
        fill="none"
      />
    );
  }
  if (badgeId === "other-other-one-of-a-kind") {
    return (
      <path
        d="M38 20l3 11 11-3-7 9 7 9-11-3-3 11-3-11-11 3 7-9-7-9 11 3z"
        fill={color}
      />
    );
  }

  // ── Maintenance - Building/Land ─────────────────────────────────────────────
  if (badgeId === "maintenance-building-land-fixer-upper") {
    return (
      <>
        <circle
          cx="28"
          cy="28"
          r="9"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M34 34l18 18"
          stroke={color}
          strokeWidth="5.5"
          strokeLinecap="round"
        />
        <path
          d="M24 28h8M28 24v8"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "maintenance-building-land-ground-control") {
    return (
      <>
        <circle
          cx="38"
          cy="38"
          r="14"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M38 18v8M38 48v8M18 38h8M48 38h8"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "maintenance-building-land-brick-by-brick") {
    return (
      <>
        <rect x="20" y="24" width="14" height="8" rx="1" fill={color} />
        <rect x="36" y="24" width="14" height="8" rx="1" fill={color} />
        <rect x="24" y="34" width="14" height="8" rx="1" fill={color} />
        <rect x="40" y="34" width="14" height="8" rx="1" fill={color} />
        <rect x="20" y="44" width="14" height="8" rx="1" fill={color} />
        <rect x="36" y="44" width="14" height="8" rx="1" fill={color} />
      </>
    );
  }
  if (badgeId === "maintenance-building-land-landmark-legend") {
    return (
      <>
        <rect x="24" y="28" width="28" height="28" fill={color} />
        <rect x="28" y="20" width="20" height="8" fill={color} />
        <rect x="34" y="16" width="8" height="4" fill={color} />
        <rect x="26" y="36" width="6" height="6" fill="rgba(255,255,255,0.6)" />
        <rect x="44" y="36" width="6" height="6" fill="rgba(255,255,255,0.6)" />
        <rect
          x="33"
          y="44"
          width="10"
          height="12"
          fill="rgba(255,255,255,0.6)"
        />
      </>
    );
  }

  // ── Other - Administration ──────────────────────────────────────────────────
  if (badgeId === "other-administration-paper-trail") {
    return (
      <>
        <path
          d="M28 20h18l8 8v28H28z"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path d="M46 20v8h8" stroke={color} strokeWidth="3.5" fill="none" />
        <path
          d="M34 34h16M34 40h16M34 46h10"
          stroke={color}
          strokeWidth="3"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "other-administration-form-force") {
    return (
      <>
        <rect
          x="26"
          y="26"
          width="24"
          height="30"
          rx="3"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M33 26v-4c0-2 1-3 3-3h4c2 0 3 1 3 3v4"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinecap="round"
        />
        <path
          d="M32 36h12M32 42h12M32 48h8"
          stroke={color}
          strokeWidth="3"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "other-administration-clerically-speaking") {
    return (
      <>
        <path d="M38 54l-6-28 6-6 6 6z" fill={color} />
        <path
          d="M32 26l6-6 6 6"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <path
          d="M32 26l6 28M44 26l-6 28"
          stroke={color}
          strokeWidth="2"
          strokeLinecap="round"
        />
        <circle cx="38" cy="56" r="2.5" fill={color} />
      </>
    );
  }
  if (badgeId === "other-administration-admin-istrator") {
    return (
      <>
        <circle cx="38" cy="38" r="7" fill={color} />
        <path
          d="M38 20v5M38 51v5M20 38h5M51 38h5M24.4 24.4l3.5 3.5M48.1 48.1l3.5 3.5M24.4 51.6l3.5-3.5M48.1 27.9l3.5-3.5"
          stroke={color}
          strokeWidth="4.5"
          strokeLinecap="round"
        />
      </>
    );
  }

  // ── myAvailability Usage ────────────────────────────────────────────────────
  if (badgeId === "myavailability-usage-now-you-see-me") {
    return (
      <>
        <path
          d="M14 38c0 0 8-16 24-16s24 16 24 16-8 16-24 16S14 38 14 38z"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <circle cx="38" cy="38" r="7" fill={color} />
        <circle cx="40" cy="36" r="2.5" fill="rgba(255,255,255,0.5)" />
      </>
    );
  }
  if (badgeId === "myavailability-usage-digitally-aware") {
    return (
      <>
        <rect
          x="28"
          y="18"
          width="20"
          height="40"
          rx="4"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <circle cx="38" cy="52" r="2.5" fill={color} />
        <path
          d="M34 24h8"
          stroke={color}
          strokeWidth="3"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "myavailability-usage-slot-machine") {
    return (
      <>
        <rect
          x="22"
          y="26"
          width="32"
          height="28"
          rx="4"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M28 18v8M48 18v8"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
        <path d="M22 36h32" stroke={color} strokeWidth="3.5" />
        <path
          d="M28 44l7 7 12-12"
          stroke={color}
          strokeWidth="4"
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </>
    );
  }
  if (badgeId === "myavailability-usage-availability-ace") {
    return (
      <>
        <rect
          x="22"
          y="26"
          width="32"
          height="28"
          rx="4"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M28 18v8M48 18v8"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
        <path d="M22 36h32" stroke={color} strokeWidth="3.5" />
        <path d="M38 40l2 5h5l-4 3 2 5-5-3-5 3 2-5-4-3h5z" fill={color} />
      </>
    );
  }

  // ── Training - PIARO ───────────────────────────────────────────────────────
  if (badgeId === "training-piaro-situation-aware") {
    return (
      <>
        <path
          d="M38 20l10 8v18l-10 10-10-10V28z"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinejoin="round"
        />
        <path
          d="M38 32v12M32 38h12"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "training-piaro-map-whisperer") {
    return (
      <>
        <path
          d="M20 26l12-4 12 4 12-4v28l-12 4-12-4-12 4z"
          stroke={color}
          strokeWidth="3"
          fill="none"
          strokeLinejoin="round"
        />
        <path d="M32 22v28M44 26v28" stroke={color} strokeWidth="3" />
      </>
    );
  }
  if (badgeId === "training-piaro-intel-operative") {
    return (
      <>
        <circle cx="30" cy="32" r="6" fill={color} />
        <circle cx="46" cy="32" r="6" fill={color} />
        <path
          d="M24 46c3-5 8-8 14-8s11 3 14 8"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "training-piaro-piaro-powerhouse") {
    return (
      <>
        <path
          d="M38 18l7 14 16 2-12 10 4 14-15-8-15 8 4-14-12-10 16-2z"
          fill={color}
        />
      </>
    );
  }

  // ── Training - Operate Comms. Equip. ─────────────────────────────────────
  if (badgeId === "training-operate-comms-can-you-read-me") {
    return (
      <>
        <rect
          x="26"
          y="24"
          width="24"
          height="28"
          rx="4"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M32 32h12M32 38h9"
          stroke={color}
          strokeWidth="3"
          strokeLinecap="round"
        />
        <circle cx="38" cy="46" r="2.5" fill={color} />
      </>
    );
  }
  if (badgeId === "training-operate-comms-loud-and-clear") {
    return (
      <>
        <rect x="20" y="32" width="16" height="12" rx="2" fill={color} />
        <path
          d="M40 32c6 1 10 4 10 6s-4 5-10 6"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinecap="round"
        />
        <path
          d="M44 28c8 2 14 6 14 10s-6 8-14 10"
          stroke={color}
          strokeWidth="2.5"
          fill="none"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "training-operate-comms-radio-royalty") {
    return (
      <>
        <circle
          cx="38"
          cy="40"
          r="14"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M38 20v8"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
        <path
          d="M30 44l6 6 10-12"
          stroke={color}
          strokeWidth="4"
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </>
    );
  }
  if (badgeId === "training-operate-comms-comms-commander") {
    return (
      <>
        <path d="M24 46h28v8H24z" fill={color} />
        <path
          d="M28 46V30a10 10 0 0 1 20 0v16"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <circle cx="38" cy="24" r="4" fill={color} />
      </>
    );
  }

  // ── Training - Field Core Skills ──────────────────────────────────────────
  if (badgeId === "training-field-core-skills-boots-on") {
    return (
      <>
        <path d="M22 44h16l4 8H22z" fill={color} />
        <path d="M34 34h16l4 18H34z" fill={color} />
      </>
    );
  }
  if (badgeId === "training-field-core-skills-bush-ready") {
    return (
      <>
        <path d="M24 52l14-26 14 26z" fill={color} />
        <path
          d="M38 30v10"
          stroke="rgba(255,255,255,0.75)"
          strokeWidth="3"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "training-field-core-skills-field-tested") {
    return (
      <>
        <path
          d="M24 38l8 8 20-20"
          stroke={color}
          strokeWidth="5"
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <rect
          x="20"
          y="20"
          width="36"
          height="36"
          rx="6"
          stroke={color}
          strokeWidth="3"
          fill="none"
        />
      </>
    );
  }
  if (badgeId === "training-field-core-skills-built-different") {
    return (
      <>
        <path
          d="M38 18l16 10v20L38 58 22 48V28z"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinejoin="round"
        />
        <path
          d="M30 40h16"
          stroke={color}
          strokeWidth="4"
          strokeLinecap="round"
        />
      </>
    );
  }

  // ── Maintenance - Equipment ───────────────────────────────────────────────
  if (badgeId === "maintenance-equipment-tool-time") {
    return (
      <>
        <path
          d="M24 24l8 8"
          stroke={color}
          strokeWidth="5"
          strokeLinecap="round"
        />
        <path
          d="M34 30l18 18"
          stroke={color}
          strokeWidth="5"
          strokeLinecap="round"
        />
        <circle cx="24" cy="24" r="4" fill={color} />
      </>
    );
  }
  if (badgeId === "maintenance-equipment-grease-monkey") {
    return (
      <>
        <circle
          cx="38"
          cy="38"
          r="12"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M38 20v6M38 50v6M20 38h6M50 38h6M27 27l4 4M45 45l4 4M27 49l4-4M45 31l4-4"
          stroke={color}
          strokeWidth="3"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "maintenance-equipment-maintenance-maestro") {
    return (
      <>
        <path
          d="M22 50l10-10 6 6 12-18"
          stroke={color}
          strokeWidth="4.5"
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <path
          d="M48 28h8v8"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "maintenance-equipment-keeper-of-the-kit") {
    return (
      <>
        <rect
          x="22"
          y="24"
          width="32"
          height="26"
          rx="4"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M30 24v-4h16v4"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinecap="round"
        />
        <path d="M22 38h32" stroke={color} strokeWidth="3.5" />
      </>
    );
  }

  // ── Training - Other ──────────────────────────────────────────────────────
  if (badgeId === "training-other-side-quest") {
    return <circle cx="38" cy="38" r="9" fill={color} />;
  }
  if (badgeId === "training-other-bonus-round") {
    return (
      <>
        <circle cx="28" cy="38" r="6" fill={color} />
        <circle cx="48" cy="38" r="6" fill={color} />
        <path
          d="M32 38h12"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "training-other-extra-credit") {
    return (
      <>
        <path
          d="M24 52l14-28 14 28z"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M32 40h12"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "training-other-secret-syllabus") {
    return (
      <path
        d="M38 20l6 12 14 2-10 9 2 13-12-6-12 6 2-13-10-9 14-2z"
        fill={color}
      />
    );
  }

  // ── Other - Unit Meeting/Muster ───────────────────────────────────────────
  if (badgeId === "unit-meeting-muster-present") {
    return (
      <>
        <circle cx="38" cy="28" r="6" fill={color} />
        <path d="M28 52c0-7 5-12 10-12s10 5 10 12" fill={color} />
      </>
    );
  }
  if (badgeId === "unit-meeting-muster-regular-face") {
    return (
      <>
        <circle cx="26" cy="30" r="5" fill={color} />
        <circle cx="38" cy="28" r="6" fill={color} />
        <circle cx="50" cy="30" r="5" fill={color} />
        <path
          d="M20 52c0-7 5-11 11-11M57 52c0-7-5-11-11-11M28 52c0-8 4-12 10-12s10 4 10 12"
          stroke={color}
          strokeWidth="3"
          fill="none"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "unit-meeting-muster-muster-master") {
    return (
      <>
        <rect
          x="20"
          y="24"
          width="36"
          height="28"
          rx="4"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M26 34h24M26 42h14"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "unit-meeting-muster-roll-call-royalty") {
    return (
      <>
        <path
          d="M24 52V24h28v28"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M30 24l8-8 8 8"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinejoin="round"
        />
        <path
          d="M30 40h16"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
      </>
    );
  }

  // ── Trainer - PIARO ───────────────────────────────────────────────────────
  if (badgeId === "trainer-piaro-passing-it-on") {
    return (
      <>
        <path
          d="M22 44l12-10 8 6 12-14"
          stroke={color}
          strokeWidth="4.5"
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <circle cx="22" cy="44" r="3" fill={color} />
      </>
    );
  }
  if (badgeId === "trainer-piaro-knowledge-broker") {
    return (
      <>
        <rect
          x="22"
          y="24"
          width="14"
          height="28"
          rx="2"
          stroke={color}
          strokeWidth="3"
          fill="none"
        />
        <rect
          x="40"
          y="24"
          width="14"
          height="28"
          rx="2"
          stroke={color}
          strokeWidth="3"
          fill="none"
        />
        <path d="M36 38h4" stroke={color} strokeWidth="3" />
      </>
    );
  }
  if (badgeId === "trainer-piaro-mission-mentor") {
    return (
      <>
        <path
          d="M20 30h36l-10 10v10H30V40z"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinejoin="round"
        />
        <circle cx="38" cy="36" r="3" fill={color} />
      </>
    );
  }
  if (badgeId === "trainer-piaro-oracle-of-operations") {
    return (
      <>
        <path d="M38 18l16 10v20L38 58 22 48V28z" fill={color} />
        <circle cx="38" cy="38" r="6" fill="rgba(255,255,255,0.65)" />
      </>
    );
  }

  // ── Trainer - Operate Comms. Equip. ──────────────────────────────────────
  if (badgeId === "trainer-operate-comms-mic-check") {
    return (
      <>
        <rect
          x="32"
          y="22"
          width="12"
          height="18"
          rx="6"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M26 34c0 7 5 12 12 12s12-5 12-12"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M38 46v8"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "trainer-operate-comms-airwave-instructor") {
    return (
      <>
        <path
          d="M22 38h12"
          stroke={color}
          strokeWidth="4"
          strokeLinecap="round"
        />
        <path
          d="M42 30c4 2 6 5 6 8s-2 6-6 8"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinecap="round"
        />
        <path
          d="M46 24c7 4 10 8 10 14s-3 10-10 14"
          stroke={color}
          strokeWidth="2.5"
          fill="none"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "trainer-operate-comms-signal-coach") {
    return (
      <>
        <path
          d="M18 50l20-26 20 26"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinejoin="round"
        />
        <circle cx="38" cy="40" r="4" fill={color} />
      </>
    );
  }
  if (badgeId === "trainer-operate-comms-voice-of-experience") {
    return (
      <>
        <circle
          cx="38"
          cy="38"
          r="14"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M30 44l8-8 8 8"
          stroke={color}
          strokeWidth="4"
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </>
    );
  }

  // ── Training - Traffic Safety ─────────────────────────────────────────────
  if (badgeId === "training-traffic-safety-cone-ranger") {
    return (
      <>
        <path d="M38 20l12 30H26z" fill={color} />
        <path
          d="M30 40h16"
          stroke="rgba(255,255,255,0.7)"
          strokeWidth="3.5"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "training-traffic-safety-traffic-tamer") {
    return (
      <>
        <path
          d="M24 50h28"
          stroke={color}
          strokeWidth="4"
          strokeLinecap="round"
        />
        <path
          d="M28 50V24h20v26"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <circle cx="38" cy="30" r="3" fill={color} />
      </>
    );
  }
  if (badgeId === "training-traffic-safety-flow-controller") {
    return (
      <>
        <path
          d="M18 28h40"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
        <path
          d="M18 38h32"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
        <path
          d="M18 48h24"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "training-traffic-safety-master-of-merge") {
    return (
      <>
        <path
          d="M22 50c8 0 8-26 16-26s8 26 16 26"
          stroke={color}
          strokeWidth="4.5"
          fill="none"
          strokeLinecap="round"
        />
        <path
          d="M28 50h20"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
      </>
    );
  }

  // ── Other - Attend Exercise - SES ─────────────────────────────────────────
  if (badgeId === "attend-exercise-ses-practice-makes") {
    return (
      <>
        <path
          d="M24 50l14-28 14 28"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <circle cx="38" cy="42" r="3" fill={color} />
      </>
    );
  }
  if (badgeId === "attend-exercise-ses-exercise-enthusiast") {
    return (
      <>
        <circle cx="30" cy="30" r="7" fill={color} />
        <circle cx="46" cy="30" r="7" fill={color} />
        <circle cx="38" cy="46" r="7" fill={color} />
      </>
    );
  }
  if (badgeId === "attend-exercise-ses-simulation-specialist") {
    return (
      <>
        <rect
          x="20"
          y="22"
          width="36"
          height="30"
          rx="4"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M28 32h20M28 40h12"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "attend-exercise-ses-exercise-extraordinaire") {
    return (
      <>
        <path
          d="M38 18l14 10v20L38 58 24 48V28z"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
          strokeLinejoin="round"
        />
        <path
          d="M30 40l6 6 10-12"
          stroke={color}
          strokeWidth="4"
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </>
    );
  }

  // ── Community Ed. & Media ─────────────────────────────────────────────────
  if (badgeId === "community-ed-media-friendly-face") {
    return (
      <>
        <circle
          cx="38"
          cy="34"
          r="11"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <circle cx="34" cy="31" r="1.8" fill={color} />
        <circle cx="42" cy="31" r="1.8" fill={color} />
        <path
          d="M33 38c2 2 6 2 8 0"
          stroke={color}
          strokeWidth="2.8"
          fill="none"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "community-ed-media-community-connector") {
    return (
      <>
        <circle cx="26" cy="34" r="6" fill={color} />
        <circle cx="50" cy="34" r="6" fill={color} />
        <path
          d="M32 34h12"
          stroke={color}
          strokeWidth="4"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "community-ed-media-public-presence") {
    return (
      <>
        <circle
          cx="38"
          cy="34"
          r="12"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M26 34h24M38 22a20 20 0 0 0 0 24M38 22a20 20 0 0 1 0 24"
          stroke={color}
          strokeWidth="3"
          fill="none"
        />
      </>
    );
  }
  if (badgeId === "community-ed-media-local-legend") {
    return (
      <>
        <path
          d="M38 20l6 12 14 2-10 9 2 13-12-6-12 6 2-13-10-9 14-2z"
          fill={color}
        />
      </>
    );
  }

  // ── Easter Eggs ────────────────────────────────────────────────────────────
  if (badgeId === "easter-just-five-minutes") {
    return (
      <>
        <circle
          cx="38"
          cy="38"
          r="14"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M38 38l8-6"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
        <path
          d="M38 28v10"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "easter-midnight-oil") {
    return (
      <>
        <circle cx="34" cy="32" r="10" fill={color} />
        <circle cx="40" cy="28" r="10" fill="rgba(255,255,255,0.7)" />
        <path d="M50 18l2 4 4 2-4 2-2 4-2-4-4-2 4-2z" fill={color} />
      </>
    );
  }
  if (badgeId === "easter-coffee-powered") {
    return (
      <>
        <path
          d="M24 30h24v16a8 8 0 0 1-8 8H32a8 8 0 0 1-8-8z"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M48 34h4a4 4 0 0 1 0 8h-4"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M30 22c0-3 2-3 2-6M38 22c0-3 2-3 2-6"
          stroke={color}
          strokeWidth="2.5"
          fill="none"
          strokeLinecap="round"
        />
      </>
    );
  }
  if (badgeId === "easter-habit-formed") {
    return (
      <>
        <rect
          x="22"
          y="24"
          width="32"
          height="28"
          rx="4"
          stroke={color}
          strokeWidth="3.5"
          fill="none"
        />
        <path
          d="M30 18v8M46 18v8"
          stroke={color}
          strokeWidth="3.5"
          strokeLinecap="round"
        />
        <path
          d="M28 38l6 6 14-14"
          stroke={color}
          strokeWidth="4"
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </>
    );
  }

  return <circle cx="38" cy="38" r="8" fill={color} />;
}

export function BadgeIcon({
  badgeId,
  badgeName,
  tier,
  className,
}: BadgeIconProps) {
  const id = useId();
  const normalizedTier = normalizeTier(tier);
  const palette = paletteForTier(normalizedTier);
  const gradientId = `badge-gradient-${id}`;
  const composedClassName = [
    className,
    "badge-icon-svg",
    `badge-icon-tier-${normalizedTier}`,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <svg
      className={composedClassName}
      viewBox="0 0 76 76"
      aria-hidden="true"
      focusable="false"
    >
      <defs>
        <linearGradient id={gradientId} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor={palette.outer} />
          <stop offset="100%" stopColor={palette.inner} />
        </linearGradient>
      </defs>
      <circle
        cx="38"
        cy="38"
        r="34"
        fill={`url(#${gradientId})`}
        stroke={palette.ring}
        strokeWidth="4"
      />
      <circle cx="38" cy="38" r="28" fill="rgba(255,255,255,0.22)" />
      <SymbolForBadge
        badgeId={badgeId}
        badgeName={badgeName}
        color={palette.symbol}
      />
    </svg>
  );
}

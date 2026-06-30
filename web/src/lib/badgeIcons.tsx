import { useId } from "react";

type BadgeTier = "starter" | "bronze" | "silver" | "gold";

type BadgeIconProps = {
  badgeId: string;
  tier?: string;
  className?: string;
};

type Palette = {
  outer: string;
  inner: string;
  ring: string;
  symbol: string;
};

function paletteForTier(tier: BadgeTier): Palette {
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
  if (tier === "gold" || tier === "silver" || tier === "bronze") {
    return tier;
  }
  return "starter";
}

function SymbolForBadge({
  badgeId,
  color,
}: {
  badgeId: string;
  color: string;
}) {
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

  // ── Trainer - Storm & Water ─────────────────────────────────────────────────
  if (badgeId === "trainer-storm-water-splash-class") {
    return (
      <path
        d="M38 20c0 0-13 14-13 22 0 7 6 12 13 12s13-5 13-12c0-8-13-22-13-22z"
        fill={color}
      />
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

  return <circle cx="38" cy="38" r="8" fill={color} />;
}

export function BadgeIcon({ badgeId, tier, className }: BadgeIconProps) {
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
      <SymbolForBadge badgeId={badgeId} color={palette.symbol} />
    </svg>
  );
}

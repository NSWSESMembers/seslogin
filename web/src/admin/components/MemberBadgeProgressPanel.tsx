import { useState } from "react";
import { BadgeIcon } from "../../lib/badgeIcons";
import { formatFullDateTime } from "../../lib/time";

type BadgeProgressItem = {
  id: string;
  name: string;
  description: string;
  tier: string;
  source: string;
  earned: boolean;
  awardedAt: number | null | undefined;
};

type BadgeGroupMode = "tier" | "source";

interface MemberBadgeProgressPanelProps {
  badgeProgress: ReadonlyArray<BadgeProgressItem>;
  heading?: string;
}

const TIER_ORDER = ["gold", "silver", "bronze", "starter"] as const;
const SOURCE_ORDER = ["Check-in", "Sign-out", "Manual"] as const;

function tierRank(tier: string) {
  const index = TIER_ORDER.indexOf(
    tier.toLowerCase() as (typeof TIER_ORDER)[number],
  );
  return index === -1 ? TIER_ORDER.length : index;
}

function sourceRank(source: string) {
  const index = SOURCE_ORDER.indexOf(source as (typeof SOURCE_ORDER)[number]);
  return index === -1 ? SOURCE_ORDER.length : index;
}

function compareBadges(a: BadgeProgressItem, b: BadgeProgressItem) {
  if (a.earned !== b.earned) {
    return a.earned ? -1 : 1;
  }

  const tierDifference = tierRank(a.tier) - tierRank(b.tier);
  if (tierDifference !== 0) {
    return tierDifference;
  }

  if (a.awardedAt == null && b.awardedAt == null) {
    return a.name.localeCompare(b.name);
  }

  if (a.awardedAt == null) {
    return 1;
  }

  if (b.awardedAt == null) {
    return -1;
  }

  if (a.awardedAt !== b.awardedAt) {
    return b.awardedAt - a.awardedAt;
  }

  return a.name.localeCompare(b.name);
}

function groupBadges(
  badgeProgress: ReadonlyArray<BadgeProgressItem>,
  mode: BadgeGroupMode,
) {
  const groups = new Map<string, BadgeProgressItem[]>();

  for (const badge of badgeProgress) {
    const key = mode === "tier" ? badge.tier.toLowerCase() : badge.source;
    const badges = groups.get(key);
    if (badges) {
      badges.push(badge);
    } else {
      groups.set(key, [badge]);
    }
  }

  const orderedKeys =
    mode === "tier"
      ? TIER_ORDER.filter((tier) => groups.has(tier))
      : [...groups.keys()].sort(
          (left, right) =>
            sourceRank(left) - sourceRank(right) || left.localeCompare(right),
        );

  return orderedKeys.map((key) => ({
    key,
    label:
      mode === "tier"
        ? `${key.charAt(0).toUpperCase()}${key.slice(1)} Tier`
        : key,
    badges: (groups.get(key) ?? []).slice().sort(compareBadges),
  }));
}

export default function MemberBadgeProgressPanel({
  badgeProgress,
  heading,
}: MemberBadgeProgressPanelProps) {
  const [mode, setMode] = useState<BadgeGroupMode>("tier");
  const badgeGroups = groupBadges(badgeProgress, mode);
  const earnedBadgeCount = badgeProgress.filter((badge) => badge.earned).length;
  const totalBadgeCount = badgeProgress.length;

  return (
    <section className="member-badges-panel">
      {heading ? <h3>{heading}</h3> : null}
      <div
        className="member-badges-summary"
        aria-label="Badge progress summary"
      >
        <div className="member-badges-summary-item">
          <span className="member-badges-summary-value">
            {earnedBadgeCount}
          </span>
          <span className="member-badges-summary-label">earned</span>
        </div>
        <div className="member-badges-summary-item">
          <span className="member-badges-summary-value">{totalBadgeCount}</span>
          <span className="member-badges-summary-label">total</span>
        </div>
        <div className="member-badges-summary-copy">
          Earned badges are highlighted with a filled banner and an award stamp.
        </div>
      </div>
      <div className="member-badges-toolbar">
        <span className="member-badges-toolbar-label">Group by</span>
        <div
          className="segmented-control"
          role="group"
          aria-label="Badge grouping"
        >
          <button
            type="button"
            className="segment-button"
            aria-pressed={mode === "tier"}
            onClick={() => setMode("tier")}
          >
            Tier
          </button>
          <button
            type="button"
            className="segment-button"
            aria-pressed={mode === "source"}
            onClick={() => setMode("source")}
          >
            Source
          </button>
        </div>
      </div>
      {badgeGroups.length === 0 ? (
        <p className="member-badges-empty">
          No badges are visible for this member at this location yet.
        </p>
      ) : (
        badgeGroups.map(({ key, label, badges }) => (
          <div key={key} className="member-badges-tier-section">
            <h4
              className={
                mode === "tier"
                  ? `member-badges-tier-heading tier-${key}`
                  : "member-badges-tier-heading member-badges-source-heading"
              }
            >
              {label}
            </h4>
            <div className="member-badges-grid">
              {badges.map((badge) => (
                <article
                  key={badge.id}
                  className={`member-badge-card tier-${badge.tier.toLowerCase()} ${badge.earned ? "earned" : "locked"}`}
                  aria-label={`${badge.name} badge ${badge.earned ? "earned" : "not yet earned"}`}
                >
                  <div className="member-badge-head">
                    <BadgeIcon
                      badgeId={badge.id}
                      tier={badge.tier.toLowerCase()}
                      className="member-badge-icon"
                    />
                    <div>
                      <div className="member-badge-title">{badge.name}</div>
                      <div className="member-badge-tier">{badge.tier} tier</div>
                    </div>
                  </div>
                  <div
                    className={`member-badge-status ${badge.earned ? "earned" : "locked"}`}
                  >
                    {badge.earned ? "Awarded" : "Locked"}
                  </div>
                  <p className="member-badge-description">
                    {badge.description}
                  </p>
                  <div className="member-badge-earned">
                    {badge.awardedAt ? (
                      <>
                        Earned{" "}
                        <strong>
                          {formatFullDateTime(new Date(badge.awardedAt * 1000))}
                        </strong>
                      </>
                    ) : (
                      "Not yet earned"
                    )}
                  </div>
                </article>
              ))}
            </div>
          </div>
        ))
      )}
    </section>
  );
}

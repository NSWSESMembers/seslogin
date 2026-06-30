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

const TIER_ORDER_DESC = [
  "gold",
  "silver",
  "bronze",
  "starter",
  "passport stamps",
] as const;
const TIER_ORDER_ASC = [
  "starter",
  "passport stamps",
  "bronze",
  "silver",
  "gold",
] as const;
const SOURCE_ORDER = [
  "Attendance",
  "Training",
  "Storm",
  "Away sign-in",
] as const;

function tierKey(tier: string) {
  return tier.trim().toLowerCase();
}

function tierCssClass(tier: string) {
  return tierKey(tier).replace(/\s+/g, "-");
}

function tierLabel(tier: string) {
  return `${tier
    .split(/\s+/)
    .filter(Boolean)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ")} Tier`;
}

function tierRank(
  tier: string,
  order: ReadonlyArray<
    "starter" | "passport stamps" | "bronze" | "silver" | "gold"
  >,
) {
  const index = order.indexOf(tierKey(tier) as (typeof TIER_ORDER_ASC)[number]);
  return index === -1 ? order.length : index;
}

function sourceRank(source: string) {
  const index = SOURCE_ORDER.indexOf(source as (typeof SOURCE_ORDER)[number]);
  return index === -1 ? SOURCE_ORDER.length : index;
}

function compareBadges(
  a: BadgeProgressItem,
  b: BadgeProgressItem,
  mode: BadgeGroupMode,
) {
  const tierDifference =
    mode === "source"
      ? tierRank(a.tier, TIER_ORDER_ASC) - tierRank(b.tier, TIER_ORDER_ASC)
      : tierRank(a.tier, TIER_ORDER_DESC) - tierRank(b.tier, TIER_ORDER_DESC);
  if (tierDifference !== 0) {
    return tierDifference;
  }

  if (a.earned !== b.earned) {
    return a.earned ? -1 : 1;
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
    const key = mode === "tier" ? tierKey(badge.tier) : badge.source;
    const badges = groups.get(key);
    if (badges) {
      badges.push(badge);
    } else {
      groups.set(key, [badge]);
    }
  }

  const orderedKeys =
    mode === "tier"
      ? [
          ...TIER_ORDER_DESC.filter((tier) => groups.has(tier)),
          ...[...groups.keys()]
            .filter(
              (tier) =>
                !TIER_ORDER_DESC.includes(
                  tier as (typeof TIER_ORDER_DESC)[number],
                ),
            )
            .sort((left, right) => left.localeCompare(right)),
        ]
      : [...groups.keys()].sort(
          (left, right) =>
            sourceRank(left) - sourceRank(right) || left.localeCompare(right),
        );

  return orderedKeys.map((key) => ({
    key,
    label: mode === "tier" ? tierLabel(key) : key,
    badges: (groups.get(key) ?? [])
      .slice()
      .sort((a, b) => compareBadges(a, b, mode)),
  }));
}

export default function MemberBadgeProgressPanel({
  badgeProgress,
  heading,
}: MemberBadgeProgressPanelProps) {
  const [mode, setMode] = useState<BadgeGroupMode>("tier");
  const [showUnawarded, setShowUnawarded] = useState(false);
  const visibleBadgeProgress = showUnawarded
    ? badgeProgress
    : badgeProgress.filter((badge) => badge.earned);
  const badgeGroups = groupBadges(visibleBadgeProgress, mode);
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
      </div>
      <div className="member-badges-toolbar">
        <div className="member-badges-toolbar-controls">
          <div className="member-badges-toolbar-group">
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
          <div className="member-badges-toolbar-group">
            <span className="member-badges-toolbar-label">Visibility</span>
            <div
              className="segmented-control"
              role="group"
              aria-label="Badge visibility"
            >
              <button
                type="button"
                className="segment-button"
                aria-pressed={!showUnawarded}
                onClick={() => setShowUnawarded(false)}
              >
                Earned
              </button>
              <button
                type="button"
                className="segment-button"
                aria-pressed={showUnawarded}
                onClick={() => setShowUnawarded(true)}
              >
                All
              </button>
            </div>
          </div>
        </div>
      </div>
      {badgeGroups.length === 0 ? (
        <p className="member-badges-empty">
          {showUnawarded
            ? "No badges are visible for this member at this location yet."
            : "No badges earned yet for this member at this location."}
        </p>
      ) : (
        badgeGroups.map(({ key, label, badges }) => (
          <div
            key={key}
            className={
              mode === "tier"
                ? `member-badges-tier-section tier-${tierCssClass(key)}`
                : "member-badges-tier-section member-badges-source-section"
            }
          >
            <h4
              className={
                mode === "tier"
                  ? `member-badges-tier-heading tier-${tierCssClass(key)}`
                  : "member-badges-tier-heading member-badges-source-heading"
              }
            >
              {label}
            </h4>
            <div
              className={`member-badges-grid ${mode === "source" ? "member-badges-grid-source" : ""}`}
            >
              {badges.map((badge) => (
                <article
                  key={badge.id}
                  className={`member-badge-card tier-${tierCssClass(badge.tier)} ${badge.earned ? "earned" : "locked"}`}
                  aria-label={`${badge.name} badge ${badge.earned ? "earned" : "not yet earned"}`}
                >
                  <div className="member-badge-emblem">
                    <BadgeIcon
                      badgeId={badge.id}
                      badgeName={badge.name}
                      tier={tierKey(badge.tier)}
                      className="member-badge-icon"
                    />
                  </div>
                  <div className="member-badge-content">
                    <div className="member-badge-title">{badge.name}</div>
                    <div className="member-badge-tier">{badge.tier} tier</div>
                    {badge.description ? (
                      <p className="member-badge-description">
                        {badge.description}
                      </p>
                    ) : null}
                    <div className="member-badge-earned">
                      {badge.awardedAt ? (
                        <>
                          Earned{" "}
                          <strong>
                            {formatFullDateTime(
                              new Date(badge.awardedAt * 1000),
                            )}
                          </strong>
                        </>
                      ) : (
                        "Not yet earned"
                      )}
                    </div>
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

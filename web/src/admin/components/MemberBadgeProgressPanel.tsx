import { useState } from "react";
import { BadgeIcon } from "../../lib/badgeIcons";
import { formatFullDateTime } from "../../lib/time";
import {
  TIER_ORDER_DESC,
  TIER_PILL_CLASS,
  tierCssClass,
  tierKey,
  tierLabel,
} from "../../lib/badgeTiers";

type BadgeProgressItem = {
  id: string;
  badgeId: string;
  name: string;
  description: string;
  tier: string;
  source: string;
  earned: boolean;
  awardedAt: number | null | undefined;
  current: number | null | undefined;
  target: number | null | undefined;
};

type BadgeGroupMode = "tier" | "source";

interface MemberBadgeProgressPanelProps {
  badgeProgress: ReadonlyArray<BadgeProgressItem>;
  heading?: string;
}

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

const TIER_SECTION_CLASS: Record<string, string> = {
  gold: "bg-linear-to-b from-[#fffaf0] to-[#fff8e6] border-[#e2cf9a]",
  silver: "bg-linear-to-b from-[#f7fbff] to-[#f2f6fb] border-[#cdd6e0]",
  bronze: "bg-linear-to-b from-[#fff7f1] to-[#f9ede3] border-[#e2c0a4]",
  starter: "bg-linear-to-b from-[#f7fbff] to-[#eef5ff] border-[#c8d8ea]",
  "passport-stamps":
    "bg-linear-to-b from-[#f4fbf7] to-[#eaf7ef] border-[#c5e1cf]",
};
const SOURCE_SECTION_CLASS =
  "bg-linear-to-b from-[#fbfaf7] to-[#f5f0e8] border-[#e3d4be]";

const TIER_CARD_BORDER_CLASS: Record<string, string> = {
  gold: "border-[#b98a00] shadow-[inset_0_0_0_1px_rgba(185,138,0,0.2)]",
  silver: "border-[#7b8794]",
  bronze: "border-[#b06f3e]",
  starter: "border-[#9ca3af]",
  "passport-stamps":
    "border-[#3f8f68] shadow-[inset_0_0_0_1px_rgba(63,143,104,0.18)]",
};

const TIER_ICON_ANIMATION_CLASS: Record<string, string> = {
  gold: "animate-[badge-tier-gold_2.2s_ease-in-out_infinite]",
  silver: "animate-[badge-tier-silver_2.8s_ease-in-out_infinite]",
  bronze:
    "[transform-origin:center] animate-[badge-tier-bronze_4.2s_ease-in-out_infinite]",
  starter: "animate-[badge-tier-starter_3.2s_ease-in-out_infinite]",
};

const SEGMENT_BUTTON_CLASS =
  "m-0 min-w-23 cursor-pointer rounded-none border-0 bg-neutral-100 px-3 py-1.5 text-neutral-800 hover:bg-neutral-200 aria-pressed:bg-navy aria-pressed:text-white aria-pressed:hover:bg-[#2b4f97]";

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
    <section className="mt-6 rounded-[10px] border border-[#d8d8d8] bg-linear-to-b from-[#fffaf4] to-white p-3.5">
      {heading ? (
        <h3 className="m-0 mb-2.5 text-[#7f3f11]">{heading}</h3>
      ) : null}
      <div
        className="mt-2.5 mb-3.5 flex flex-wrap items-stretch gap-3 rounded-[14px] border border-[#d9b57a] bg-linear-to-br from-[#fff7e8] via-[#fff1d4] via-55% to-[#f7e2bf] p-3 shadow-[0_4px_16px_rgba(133,89,25,0.12)]"
        aria-label="Badge progress summary"
      >
        <div className="flex min-w-21 flex-col justify-center rounded-xl border border-[rgba(154,108,50,0.2)] bg-white/74 px-3 py-2">
          <span className="text-[1.6rem] leading-none font-extrabold text-[#7f3f11]">
            {earnedBadgeCount}
          </span>
          <span className="text-[0.82rem] tracking-[0.08em] text-[#7f5d36] uppercase">
            earned
          </span>
        </div>
        <div className="flex min-w-21 flex-col justify-center rounded-xl border border-[rgba(154,108,50,0.2)] bg-white/74 px-3 py-2">
          <span className="text-[1.6rem] leading-none font-extrabold text-[#7f3f11]">
            {totalBadgeCount}
          </span>
          <span className="text-[0.82rem] tracking-[0.08em] text-[#7f5d36] uppercase">
            total
          </span>
        </div>
      </div>
      <div className="mb-3.5 flex flex-wrap items-center justify-between gap-3">
        <div className="ml-auto flex flex-wrap items-center justify-end gap-3">
          <div className="inline-flex items-center gap-2">
            <span className="text-[0.92rem] font-bold text-[#6b4d33]">
              Group by
            </span>
            <div
              className="inline-flex overflow-hidden rounded-lg border border-neutral-400"
              role="group"
              aria-label="Badge grouping"
            >
              <button
                type="button"
                className={SEGMENT_BUTTON_CLASS}
                aria-pressed={mode === "tier"}
                onClick={() => setMode("tier")}
              >
                Tier
              </button>
              <button
                type="button"
                className={`${SEGMENT_BUTTON_CLASS} border-l border-neutral-400`}
                aria-pressed={mode === "source"}
                onClick={() => setMode("source")}
              >
                Source
              </button>
            </div>
          </div>
          <div className="inline-flex items-center gap-2">
            <span className="text-[0.92rem] font-bold text-[#6b4d33]">
              Visibility
            </span>
            <div
              className="inline-flex overflow-hidden rounded-lg border border-neutral-400"
              role="group"
              aria-label="Badge visibility"
            >
              <button
                type="button"
                className={SEGMENT_BUTTON_CLASS}
                aria-pressed={!showUnawarded}
                onClick={() => setShowUnawarded(false)}
              >
                Earned
              </button>
              <button
                type="button"
                className={`${SEGMENT_BUTTON_CLASS} border-l border-neutral-400`}
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
        <p className="m-0 text-[#555555]">
          {showUnawarded
            ? "No badges are visible for this member at this location yet."
            : "No badges earned yet for this member at this location."}
        </p>
      ) : (
        badgeGroups.map(({ key, label, badges }) => (
          <div
            key={key}
            className={`mb-6 rounded-2xl border px-3.5 pt-3.5 pb-4 text-center ${
              mode === "tier"
                ? (TIER_SECTION_CLASS[tierCssClass(key)] ??
                  "border-[#e4d8c9] bg-[#fbfaf7]")
                : SOURCE_SECTION_CLASS
            }`}
          >
            <h4
              className={`m-0 mb-2.5 inline-block rounded-md border px-2.5 py-1 font-title text-base tracking-[0.08em] uppercase ${
                mode === "tier"
                  ? TIER_PILL_CLASS[tierCssClass(key)]
                  : "border-[#b9aa94] bg-[#f2efe8] text-[#5b4a35]"
              }`}
            >
              {label}
            </h4>
            <div
              className={`grid justify-start gap-2.5 ${
                mode === "source"
                  ? "grid-cols-[repeat(auto-fill,minmax(180px,260px))]"
                  : "grid-cols-[repeat(auto-fill,minmax(220px,320px))]"
              }`}
            >
              {badges.map((badge) => {
                const tier = tierCssClass(badge.tier);
                const thick = mode === "source" || badge.earned;
                return (
                  <article
                    key={badge.id}
                    className={`relative box-border flex w-full flex-col items-center gap-2.5 overflow-hidden rounded-[10px] p-2.5 text-center ${thick ? "border-2" : "border"} ${TIER_CARD_BORDER_CLASS[tier] ?? "border-[#d5c0ad]"} ${
                      badge.earned
                        ? "bg-linear-to-br from-[#fff6de] to-[#ffeab9] shadow-[0_8px_22px_rgba(171,118,17,0.16)] after:absolute after:top-3 after:-right-8 after:rotate-34 after:bg-linear-to-br after:from-[#b97d00] after:to-[#e0a81a] after:px-8.5 after:py-1 after:text-[0.74rem] after:font-extrabold after:tracking-[0.08em] after:text-[#fff7df] after:uppercase after:shadow-[0_3px_10px_rgba(135,89,0,0.24)] after:content-['Awarded']"
                        : "bg-[#f6f6f6] opacity-70 grayscale-[0.2]"
                    }`}
                    aria-label={`${badge.name} badge ${badge.earned ? "earned" : "not yet earned"}`}
                  >
                    <div className="mt-1 grid size-18 place-items-center rounded-full border border-[rgba(145,101,47,0.35)] bg-[radial-gradient(circle_at_35%_30%,#ffffff_0%,#f8eedf_70%)] shadow-[inset_0_1px_0_rgba(255,255,255,0.85),0_5px_14px_rgba(107,70,21,0.18)]">
                      <BadgeIcon
                        badgeId={badge.badgeId}
                        badgeName={badge.name}
                        tier={tier}
                        className={`block size-13 ${badge.earned ? (TIER_ICON_ANIMATION_CLASS[tier] ?? "") : "grayscale-[0.28]"}`}
                      />
                    </div>
                    <div className="flex w-full flex-col items-center">
                      <div
                        className={`mb-px leading-[1.2] font-bold ${badge.earned ? "text-[#4e2f00]" : "text-[#6f6f6f]"}`}
                      >
                        {badge.name}
                      </div>
                      <div
                        className={`text-[0.88rem] capitalize ${badge.earned ? "text-[#7a4b00]" : "text-[#6f6f6f]"}`}
                      >
                        {badge.tier} tier
                      </div>
                      {badge.description ? (
                        <p
                          className={`mt-2 mb-2.5 ${badge.earned ? "text-[#493200]" : "text-[#6f6f6f]"}`}
                        >
                          {badge.description}
                        </p>
                      ) : null}
                      <div
                        className={`mt-0.5 text-[0.85rem] ${badge.earned ? "text-[#6b4200]" : "text-[#6f6f6f]"}`}
                      >
                        {badge.awardedAt ? (
                          <>
                            Earned{" "}
                            <strong>
                              {formatFullDateTime(
                                new Date(badge.awardedAt * 1000),
                              )}
                            </strong>
                          </>
                        ) : badge.target ? (
                          <div
                            className="flex w-full items-center gap-1.5"
                            role="progressbar"
                            aria-valuenow={Math.min(
                              badge.current ?? 0,
                              badge.target,
                            )}
                            aria-valuemin={0}
                            aria-valuemax={badge.target}
                            aria-label={`Progress toward ${badge.name}`}
                          >
                            <div className="h-1.25 flex-1 overflow-hidden rounded-full bg-black/10">
                              <div
                                className="h-full rounded-full bg-linear-to-r from-[#b06f3e] to-[#e0a81a]"
                                style={{
                                  width: `${Math.min(100, ((badge.current ?? 0) / badge.target) * 100)}%`,
                                }}
                              />
                            </div>
                            <span className="shrink-0 text-[0.74rem] font-bold whitespace-nowrap text-[#6a4f3a]">
                              {Math.min(badge.current ?? 0, badge.target)}/
                              {badge.target}
                            </span>
                          </div>
                        ) : (
                          "Not yet earned"
                        )}
                      </div>
                    </div>
                  </article>
                );
              })}
            </div>
          </div>
        ))
      )}
    </section>
  );
}

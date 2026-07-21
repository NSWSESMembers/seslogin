import { useKioskSession } from "./useKioskSession";
import TitleBarShell from "../../components/ui/TitleBarShell";
import { Suspense, useState } from "react";
import { graphql, useLazyLoadQuery } from "react-relay";
import type { ScanTitleBarLeaderboardQuery } from "./__generated__/ScanTitleBarLeaderboardQuery.graphql";
import type { ScanTitleBarMemberBadgesQuery } from "./__generated__/ScanTitleBarMemberBadgesQuery.graphql";
import { formatFullDateTime } from "../../lib/time";
import MemberBadgeProgressPanel from "../../admin/components/MemberBadgeProgressPanel";
import {
  TIER_ORDER_DESC,
  TIER_PILL_CLASS,
  tierCssClass,
  tierKey,
  tierLabel,
} from "../../lib/badgeTiers";

const TITLE_BAR_BUTTON_CLASS =
  "cursor-pointer rounded-[7px] border-2 border-white bg-transparent px-4 py-2.5 font-title text-[0.6em] text-white active:bg-white/20";

const MODAL_HEADER_BUTTON_CLASS =
  "rounded-lg border border-[#95afd9] bg-white px-3.5 py-2 font-bold text-[0.95rem] text-[#1c3659]";

const LEADERBOARD_OVERLAY_CLASS = "fixed inset-0 z-[95]";
const LEADERBOARD_BACKDROP_CLASS =
  "absolute inset-0 cursor-pointer border-none bg-black/58";
const LEADERBOARD_PANEL_CLASS =
  "absolute top-1/2 left-1/2 max-h-[calc(100vh-80px)] min-h-[min(560px,calc(100vh-80px))] w-[min(1080px,calc(100vw-28px))] -translate-x-1/2 -translate-y-1/2 overflow-auto rounded-2xl border border-[rgba(105,141,204,0.35)] bg-[radial-gradient(circle_at_90%_-15%,rgba(255,184,77,0.22),transparent_35%),linear-gradient(165deg,#f6fbff_0%,#eef5ff_52%,#e6f0ff_100%)] p-4.5 text-[#223041] shadow-[0_24px_60px_rgba(0,0,0,0.42)]";
const LEADERBOARD_HEADER_CLASS =
  "mb-2.5 flex items-center justify-between gap-3";
const LEADERBOARD_SUMMARY_CLASS = "mb-3 grid grid-cols-2 gap-2.5";
const LEADERBOARD_SUMMARY_ITEM_CLASS =
  "grid gap-0.5 rounded-xl border border-[rgba(126,158,211,0.45)] bg-white/72 px-3 py-2.5";
const LEADERBOARD_SUMMARY_VALUE_CLASS =
  "text-[1.35rem] font-extrabold text-[#17345c]";
const LEADERBOARD_SUMMARY_LABEL_CLASS =
  "text-[0.86rem] tracking-[0.02em] text-[#4b6282] uppercase";

const MODAL_LOADING_CLASS =
  "grid min-h-[320px] place-content-center justify-items-center gap-3.5 p-5 text-center";
const MODAL_LOADING_SPINNER_CLASS =
  "size-10.5 animate-[kiosk-modal-spin_0.9s_linear_infinite] rounded-full border-4 border-[rgba(70,107,161,0.24)] border-t-[#2f70bd] motion-reduce:animate-none";
const MODAL_LOADING_MESSAGE_CLASS = "m-0 text-base font-bold text-[#27466f]";
const MODAL_LOADING_PROGRESS_CLASS =
  "h-2.5 w-[min(360px,80%)] overflow-hidden rounded-full border border-[rgba(84,124,186,0.32)] bg-[rgba(104,140,194,0.22)]";
const MODAL_LOADING_PROGRESS_BAR_CLASS =
  "h-full w-[45%] animate-[kiosk-modal-progress-slide_1.2s_ease-in-out_infinite] rounded-[inherit] bg-linear-to-r from-[#2a6aba] to-[#6ca4ea] motion-reduce:animate-none";

function tierCountPillClass(tier: string) {
  return `inline-flex min-w-5.5 items-center justify-center rounded-full border px-2 py-0.75 text-[0.85rem] font-extrabold ${TIER_PILL_CLASS[tierCssClass(tier)]}`;
}

function LeaderboardModalLoadingFallback(props: {
  locationName: string;
  onClose: () => void;
}) {
  return (
    <div className={LEADERBOARD_OVERLAY_CLASS} role="dialog" aria-modal="true">
      <button
        className={LEADERBOARD_BACKDROP_CLASS}
        aria-label="Close leaderboard"
        onClick={props.onClose}
      />
      <div className={LEADERBOARD_PANEL_CLASS}>
        <div className={LEADERBOARD_HEADER_CLASS}>
          <h2 className="m-0 text-[1.7rem] text-[#1b355a] capitalize">
            {props.locationName} badge leaderboard
          </h2>
          <button className={MODAL_HEADER_BUTTON_CLASS} onClick={props.onClose}>
            Close
          </button>
        </div>
        <div className={LEADERBOARD_SUMMARY_CLASS} aria-hidden="true">
          <div className={LEADERBOARD_SUMMARY_ITEM_CLASS}>
            <span className={LEADERBOARD_SUMMARY_VALUE_CLASS}>...</span>
            <span className={LEADERBOARD_SUMMARY_LABEL_CLASS}>
              Members ranked
            </span>
          </div>
          <div className={LEADERBOARD_SUMMARY_ITEM_CLASS}>
            <span className={LEADERBOARD_SUMMARY_VALUE_CLASS}>...</span>
            <span className={LEADERBOARD_SUMMARY_LABEL_CLASS}>
              Total badges
            </span>
          </div>
        </div>
        <div className="min-h-[360px]">
          <div
            className={MODAL_LOADING_CLASS}
            role="status"
            aria-live="polite"
            aria-label="Loading badge leaderboard"
          >
            <div className={MODAL_LOADING_SPINNER_CLASS} aria-hidden="true" />
            <p className={MODAL_LOADING_MESSAGE_CLASS}>
              Loading leaderboard rankings...
            </p>
            <div className={MODAL_LOADING_PROGRESS_CLASS} aria-hidden="true">
              <div className={MODAL_LOADING_PROGRESS_BAR_CLASS} />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function MemberBadgesModalLoadingFallback(props: {
  personName: string;
  onClose: () => void;
}) {
  return (
    <div className="fixed inset-0 z-[96]" role="dialog" aria-modal="true">
      <button
        className="absolute inset-0 cursor-pointer border-none bg-black/56"
        aria-label="Close member badges"
        onClick={props.onClose}
      />
      <div className="absolute top-1/2 left-1/2 max-h-[calc(100vh-40px)] min-h-[min(640px,calc(100vh-40px))] w-[min(1140px,calc(100vw-24px))] -translate-x-1/2 -translate-y-1/2 overflow-auto rounded-[14px] border border-[rgba(123,155,206,0.4)] bg-linear-to-b from-[#f9fcff] to-[#edf3ff] p-4 text-[#20344f] shadow-[0_24px_60px_rgba(0,0,0,0.38)]">
        <div className="mb-2.5 flex items-start justify-between gap-3">
          <div>
            <h2 className="m-0 text-[1.6rem] text-[#173459]">
              {props.personName} badge progress
            </h2>
            <p className="mt-1 mb-0 text-[0.95rem] text-[#5b6f8a]">
              Preparing badge details...
            </p>
          </div>
          <button
            className="rounded-lg border border-[#97aed6] bg-white px-3 py-2 font-bold text-[#18345a]"
            onClick={props.onClose}
          >
            Close
          </button>
        </div>
        <div
          className={`${MODAL_LOADING_CLASS} min-h-[460px]`}
          role="status"
          aria-live="polite"
          aria-label="Loading member badge progress"
        >
          <div className={MODAL_LOADING_SPINNER_CLASS} aria-hidden="true" />
          <p className={MODAL_LOADING_MESSAGE_CLASS}>
            Loading badge progress...
          </p>
          <div className={MODAL_LOADING_PROGRESS_CLASS} aria-hidden="true">
            <div className={MODAL_LOADING_PROGRESS_BAR_CLASS} />
          </div>
        </div>
      </div>
    </div>
  );
}

function LeaderboardModal(props: {
  locationId: string;
  locationName: string;
  onClose: () => void;
}) {
  const data = useLazyLoadQuery<ScanTitleBarLeaderboardQuery>(
    graphql`
      query ScanTitleBarLeaderboardQuery($locationId: ID!, $limit: Int) {
        location(id: $locationId) {
          id
          badgeLeaderboard(limit: $limit) {
            person {
              id
              firstName
              lastName
              memberNumber
            }
            badgeCount
            recentBadgeCount7D
            latestBadgeAwardAt
            tierCounts {
              tier
              count
            }
          }
        }
      }
    `,
    { locationId: props.locationId, limit: 10 },
    { fetchKey: props.locationId },
  );

  const rows = data.location.badgeLeaderboard;
  const totalBadgesAwarded = rows.reduce((sum, row) => sum + row.badgeCount, 0);

  const overallTierCounts = new Map<string, number>();
  for (const row of rows) {
    for (const tierCount of row.tierCounts) {
      const key = tierKey(tierCount.tier);
      overallTierCounts.set(
        key,
        (overallTierCounts.get(key) ?? 0) + tierCount.count,
      );
    }
  }

  return (
    <div className={LEADERBOARD_OVERLAY_CLASS} role="dialog" aria-modal="true">
      <button
        className={LEADERBOARD_BACKDROP_CLASS}
        aria-label="Close leaderboard"
        onClick={props.onClose}
      />
      <div className={LEADERBOARD_PANEL_CLASS}>
        <div className={LEADERBOARD_HEADER_CLASS}>
          <h2 className="m-0 text-[1.7rem] text-[#1b355a] capitalize">
            {props.locationName} badge leaderboard
          </h2>
          <button className={MODAL_HEADER_BUTTON_CLASS} onClick={props.onClose}>
            Close
          </button>
        </div>
        <div className={LEADERBOARD_SUMMARY_CLASS}>
          <div className={LEADERBOARD_SUMMARY_ITEM_CLASS}>
            <span className={LEADERBOARD_SUMMARY_VALUE_CLASS}>
              {rows.length}
            </span>
            <span className={LEADERBOARD_SUMMARY_LABEL_CLASS}>
              Members ranked
            </span>
          </div>
          <div className={LEADERBOARD_SUMMARY_ITEM_CLASS}>
            <span className={LEADERBOARD_SUMMARY_VALUE_CLASS}>
              {totalBadgesAwarded}
            </span>
            <span className={LEADERBOARD_SUMMARY_LABEL_CLASS}>
              Total badges
            </span>
          </div>
        </div>
        <div className="mt-2.5 mb-3 flex flex-wrap gap-2">
          {TIER_ORDER_DESC.map((tier) => (
            <span
              key={tier}
              className={`${tierCountPillClass(tier)} min-w-0 gap-1.5 px-2.5 py-1`}
              title={tierLabel(tier)}
            >
              {overallTierCounts.get(tier) ?? 0}
              <span className="text-[0.78rem] font-semibold tracking-[0.02em] uppercase">
                {tierLabel(tier).replace(/ Tier$/, "")}
              </span>
            </span>
          ))}
        </div>
        <div className="min-h-[360px]">
          {rows.length === 0 ? (
            <p className="mt-0.5 mb-0 text-[1.05rem] text-[#3c4d64]">
              No badges earned here yet.
            </p>
          ) : (
            <ol className="m-0 grid list-none gap-2.5 p-0">
              {rows.map((row, idx) => (
                <li
                  key={row.person.id}
                  className="grid grid-cols-[62px_minmax(0,1fr)_auto] items-center gap-x-3 gap-y-2 rounded-xl border border-[rgba(121,154,206,0.4)] bg-white/82 px-3 py-2.5 [grid-template-areas:'rank_member_latest'_'rank_metrics_latest']"
                >
                  <div className="flex size-12.5 items-center justify-center rounded-full bg-linear-[140deg] from-[#2f7fd2] to-[#1b4e8f] text-base font-extrabold text-white shadow-[inset_0_0_0_2px_rgba(255,255,255,0.28)] [grid-area:rank]">
                    #{idx + 1}
                  </div>
                  <div className="min-w-0 [grid-area:member]">
                    <div className="text-[1.1rem] leading-[1.25] font-extrabold text-[#172d4b]">
                      {row.person.firstName} {row.person.lastName}
                    </div>
                    <div className="text-[0.9rem] text-[#536888]">
                      SES ID: {row.person.memberNumber || "-"}
                    </div>
                  </div>
                  <div className="flex flex-wrap gap-2 [grid-area:metrics]">
                    <span className="inline-flex gap-1">
                      {TIER_ORDER_DESC.map((tier) => {
                        const count =
                          row.tierCounts.find(
                            (tierCount) => tierKey(tierCount.tier) === tier,
                          )?.count ?? 0;
                        return (
                          <span
                            key={tier}
                            className={tierCountPillClass(tier)}
                            title={tierLabel(tier)}
                          >
                            {count}
                          </span>
                        );
                      })}
                    </span>
                    <span className="inline-flex items-center rounded-full bg-[#d8f8ff] px-2.5 py-1 text-[0.88rem] font-bold text-[#0f4d55]">
                      {row.recentBadgeCount7D} in 7d
                    </span>
                  </div>
                  <div className="grid content-center justify-items-end [grid-area:latest]">
                    <span className="text-[0.72rem] tracking-[0.06em] text-[#68809f] uppercase">
                      Latest
                    </span>
                    <span className="text-[0.9rem] font-semibold text-[#1f3350]">
                      {formatFullDateTime(
                        new Date(row.latestBadgeAwardAt * 1000),
                      )}
                    </span>
                  </div>
                </li>
              ))}
            </ol>
          )}
        </div>
      </div>
    </div>
  );
}

function MemberBadgesModal(props: {
  personId: string;
  locationId: string;
  personName: string;
  onClose: () => void;
}) {
  /* eslint-disable relay/unused-fields */
  const data = useLazyLoadQuery<ScanTitleBarMemberBadgesQuery>(
    graphql`
      query ScanTitleBarMemberBadgesQuery($id: ID!, $locationId: ID!) {
        person(id: $id) {
          id
          memberNumber
          badgeProgress(locationId: $locationId) {
            id
            badgeId
            name
            description
            tier
            source
            earned
            awardedAt
            current
            target
          }
        }
      }
    `,
    { id: props.personId, locationId: props.locationId },
    { fetchKey: `${props.personId}-${props.locationId}` },
  );
  /* eslint-enable relay/unused-fields */

  const person = data.person;

  return (
    <div className="fixed inset-0 z-[96]" role="dialog" aria-modal="true">
      <button
        className="absolute inset-0 cursor-pointer border-none bg-black/56"
        aria-label="Close member badges"
        onClick={props.onClose}
      />
      <div className="absolute top-1/2 left-1/2 max-h-[calc(100vh-40px)] min-h-[min(640px,calc(100vh-40px))] w-[min(1140px,calc(100vw-24px))] -translate-x-1/2 -translate-y-1/2 overflow-auto rounded-[14px] border border-[rgba(123,155,206,0.4)] bg-linear-to-b from-[#f9fcff] to-[#edf3ff] p-4 text-[#20344f] shadow-[0_24px_60px_rgba(0,0,0,0.38)]">
        <div className="mb-2.5 flex items-start justify-between gap-3">
          <div>
            <h2 className="m-0 text-[1.6rem] text-[#173459]">
              {props.personName} badge progress
            </h2>
            <p className="mt-1 mb-0 text-[0.95rem] text-[#5b6f8a]">
              {person.memberNumber
                ? `SES ID ${person.memberNumber}`
                : "No SES ID"}
            </p>
          </div>
          <button
            className="rounded-lg border border-[#97aed6] bg-white px-3 py-2 font-bold text-[#18345a]"
            onClick={props.onClose}
          >
            Close
          </button>
        </div>
        <MemberBadgeProgressPanel badgeProgress={person.badgeProgress} />
      </div>
    </div>
  );
}

export default function ScanTitleBar(props: {
  onCancelSignOut?: () => void;
  signingOutName?: string;
  signingOutPersonId?: string;
}) {
  const session = useKioskSession();
  const [showLeaderboard, setShowLeaderboard] = useState(false);
  const [showMemberBadges, setShowMemberBadges] = useState(false);
  const locationName = session?.location.name ?? "Unknown location";
  const locationId = session?.location.id;
  const gamificationEnabled = session?.location.gamificationEnabled ?? false;
  const sessionName = session?.name ?? "Unknown kiosk";
  const title = props.signingOutName
    ? `${locationName} > ${sessionName} > ${props.signingOutName}`
    : `${locationName} > ${sessionName}`;

  return (
    <TitleBarShell>
      <span>{title}</span>
      <div className="ml-auto flex items-center gap-2.5">
        {locationId && gamificationEnabled && (
          <button
            className={TITLE_BAR_BUTTON_CLASS}
            onClick={() => setShowLeaderboard(true)}
          >
            Badge leaderboard
          </button>
        )}
        {locationId && gamificationEnabled && props.signingOutPersonId && (
          <button
            className={TITLE_BAR_BUTTON_CLASS}
            onClick={() => setShowMemberBadges(true)}
          >
            View member badges
          </button>
        )}
        {props.onCancelSignOut && (
          <button
            onClick={props.onCancelSignOut}
            className={TITLE_BAR_BUTTON_CLASS}
          >
            Cancel sign out
          </button>
        )}
      </div>
      {showLeaderboard && locationId && gamificationEnabled && (
        <Suspense
          fallback={
            <LeaderboardModalLoadingFallback
              locationName={locationName}
              onClose={() => setShowLeaderboard(false)}
            />
          }
        >
          <LeaderboardModal
            locationId={locationId}
            locationName={locationName}
            onClose={() => setShowLeaderboard(false)}
          />
        </Suspense>
      )}
      {showMemberBadges &&
        locationId &&
        gamificationEnabled &&
        props.signingOutPersonId && (
          <Suspense
            fallback={
              <MemberBadgesModalLoadingFallback
                personName={props.signingOutName ?? "Member"}
                onClose={() => setShowMemberBadges(false)}
              />
            }
          >
            <MemberBadgesModal
              personId={props.signingOutPersonId}
              locationId={locationId}
              personName={props.signingOutName ?? "Member"}
              onClose={() => setShowMemberBadges(false)}
            />
          </Suspense>
        )}
    </TitleBarShell>
  );
}

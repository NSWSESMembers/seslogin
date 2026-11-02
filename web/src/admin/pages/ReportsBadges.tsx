import { graphql } from "react-relay";
import { TextLink } from "../../components/ui/TextLink";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import useSelectedLocation from "../components/useSelectedLocation";
import { formatFullDateTime } from "../../lib/time";
import { competitionRanks } from "../../lib/competitionRank";
import type { ReportsBadgesQuery } from "./__generated__/ReportsBadgesQuery.graphql";
import { AdminTable, Th, Td } from "../../components/ui/Table";
import { SectionHeading } from "../../components/ui/SectionHeading";

type ReportLocation = NonNullable<
  NonNullable<ReportsBadgesQuery["response"]>["location"]
>;
type ReportPerson = ReportLocation["people"][number];
type ReportTrophy = ReportLocation["trophies"][number];
type ReportReign = ReportTrophy["reigns"][number];

const trophyDateFormatter = new Intl.DateTimeFormat(undefined, {
  day: "numeric",
  month: "short",
  year: "numeric",
});

function formatTrophyDate(seconds: number) {
  return trophyDateFormatter.format(new Date(seconds * 1000));
}

function personName(
  person: { firstName: string; lastName: string } | null | undefined,
) {
  return person ? `${person.firstName} ${person.lastName}` : "A former member";
}

function reignEnding(reign: ReportReign) {
  switch (reign.endReason) {
    case "STOLEN":
      return `taken by ${personName(reign.endedBy)}`;
    case "AGED_OUT":
      return reign.endedBy
        ? `claim aged out; ${personName(reign.endedBy)} took over`
        : "claim aged out";
    case "REVERTED":
      return reign.endedBy
        ? `claim no longer stood; passed to ${personName(reign.endedBy)}`
        : "claim no longer stood";
    case "LEFT_UNIT":
      return reign.endedBy
        ? `left the unit; passed to ${personName(reign.endedBy)}`
        : "left the unit";
    default:
      return "ended";
  }
}

// The shield's outline colour, as in its SVG.
const TROPHY_COLOUR = "#9a6d12";

/**
 * The trophy's shield (web/public/image/badges/by-id/trophy-<id>.svg) with the
 * unit's name on a ribbon across it: the trophy belongs to the unit.
 */
function TrophyShield({
  trophyId,
  unitName,
}: {
  trophyId: string;
  unitName: string;
}) {
  const label = unitName.toUpperCase();
  // The ribbon holds about 16 characters at full size; shrink longer names.
  const fontSize = Math.min(9.5, 150 / Math.max(label.length, 1));
  return (
    <div className="relative size-32.5">
      <img
        src={`/image/badges/by-id/trophy-${trophyId}.svg`}
        alt=""
        width={130}
        height={130}
        className="size-full"
      />
      <svg
        viewBox="-10 0 140 140"
        className="absolute inset-0 size-full"
        aria-hidden
      >
        <path
          d="M2 92 H118 L112 101 L118 110 H2 L8 101 Z"
          fill={TROPHY_COLOUR}
        />
        <text
          x="60"
          y={101 + fontSize * 0.36}
          textAnchor="middle"
          fontFamily="ui-monospace, Menlo, Consolas, monospace"
          fontSize={fontSize}
          fontWeight={600}
          letterSpacing={fontSize * 0.14}
          fill="#ffffff"
        >
          {label}
        </text>
      </svg>
    </div>
  );
}

function TrophyCard({
  trophy,
  unitName,
}: {
  trophy: ReportTrophy;
  unitName: string;
}) {
  return (
    <article className="flex flex-col gap-2 rounded-xl border border-[#d7c3b1] bg-linear-to-b from-[#fff9f1] to-white px-4 py-3.5 dark:border-line dark:from-surface-raised dark:to-surface-raised">
      <div className="flex flex-col items-center gap-1 text-center">
        <TrophyShield trophyId={trophy.id} unitName={unitName} />
        <div className="font-title text-xl text-navy dark:text-ink">
          {trophy.name}
        </div>
        <div className="text-sm text-ink-muted">{trophy.tagline}</div>
      </div>
      {trophy.holder ? (
        <div className="text-center">
          <TextLink to={`/admin/members/badges/${trophy.holder.id}`}>
            {personName(trophy.holder)}
          </TextLink>
          <div className="text-sm text-ink-muted">
            {trophy.valueLabel}
            {trophy.heldSince != null
              ? ` · since ${formatTrophyDate(trophy.heldSince)}`
              : ""}
          </div>
        </div>
      ) : (
        <div className="text-center text-sm text-ink-muted">
          Nobody holds it yet. It takes at least {trophy.minimumLabel}.
        </div>
      )}
      <details className="group/rule text-sm">
        <summary className="cursor-pointer list-none text-center text-ink-muted [&::-webkit-details-marker]:hidden">
          <span
            aria-hidden
            className="mr-1 inline-block transition-transform group-open/rule:rotate-90"
          >
            ▸
          </span>
          How to win it
          {trophy.reigns.length > 0 ? " and earlier holders" : ""}
        </summary>
        <p className="mt-1">
          {trophy.rule} At least {trophy.minimumLabel}.
        </p>
        {trophy.reigns.length > 0 && (
          <ol className="mt-2 flex flex-col gap-1.5 border-t border-line pt-2">
            {trophy.reigns.map((reign) => (
              <li key={`${reign.startedAt}-${reign.endedAt}`}>
                <span className="font-semibold">
                  {personName(reign.holder)}
                </span>{" "}
                · {reign.valueLabel}
                <div className="text-ink-muted">
                  {formatTrophyDate(reign.startedAt)} to{" "}
                  {formatTrophyDate(reign.endedAt)}, {reignEnding(reign)}
                </div>
              </li>
            ))}
          </ol>
        )}
      </details>
    </article>
  );
}

function TrophyCabinet({
  trophies,
  unitName,
}: {
  trophies: ReadonlyArray<ReportTrophy>;
  unitName: string;
}) {
  return (
    <section className="my-6">
      <SectionHeading>Perpetual trophies</SectionHeading>
      <p className="mx-auto my-2 max-w-[68ch]">
        Each trophy belongs to the unit and has one holder at a time. Whoever
        beats the holder takes it. Most are worked out overnight; The Boomerang
        and The Odometer can change hands when a member signs in.
      </p>
      <div className="grid grid-cols-[repeat(auto-fill,minmax(230px,1fr))] gap-3">
        {trophies.map((trophy) => (
          <TrophyCard key={trophy.id} trophy={trophy} unitName={unitName} />
        ))}
      </div>
    </section>
  );
}

const RECENT_AWARDS_CUTOFF_SECONDS =
  Math.floor(Date.now() / 1000) - 7 * 24 * 60 * 60;

function sortBadgesDescending(person: ReportPerson) {
  return [...person.badges].sort((a, b) => b.awardedAt - a.awardedAt);
}

function countRecentAwards(person: ReportPerson, cutoffSeconds: number) {
  return person.badges.filter((badge) => badge.awardedAt >= cutoffSeconds)
    .length;
}

export default function ReportsBadges() {
  const selectedLocation = useSelectedLocation();
  const locationId = selectedLocation.id;
  const data = useRetryableLazyLoadQuery<ReportsBadgesQuery>(
    graphql`
      query ReportsBadgesQuery($location: ID!) @throwOnFieldError {
        location(id: $location) {
          id
          people {
            id
            firstName
            lastName
            memberNumber
            badges(locationId: $location) {
              id
              name
              awardedAt
            }
          }
          trophies {
            id
            name
            tagline
            rule
            minimumLabel
            holder {
              id
              firstName
              lastName
            }
            valueLabel
            heldSince
            reigns(first: 5) {
              holder {
                firstName
                lastName
              }
              valueLabel
              startedAt
              endedAt
              endReason
              endedBy {
                firstName
                lastName
              }
            }
          }
        }
      }
    `,
    { location: locationId },
    { fetchKey: locationId },
  );

  if (!selectedLocation.gamificationEnabled) {
    return (
      <>
        <p className="my-4">Gamification is disabled for this location.</p>
        <p className="my-4">
          <TextLink to="/admin/reports">Back to reports</TextLink>
        </p>
      </>
    );
  }

  const location = data.location;
  const people = [...location.people]
    .map((person) => ({
      person,
      badges: sortBadgesDescending(person),
      recentCount: countRecentAwards(person, RECENT_AWARDS_CUTOFF_SECONDS),
    }))
    .filter(({ badges }) => badges.length > 0)
    .sort((a, b) => {
      if (b.badges.length !== a.badges.length) {
        return b.badges.length - a.badges.length;
      }
      return b.badges[0].awardedAt - a.badges[0].awardedAt;
    });

  // Members with the same badge count share a place (1, 1, 3), as on the
  // dashboard and kiosk leaderboards; latest award only orders rows in a tie.
  const ranks = competitionRanks(people, (entry) => entry.badges.length);

  const totalAwards = people.reduce(
    (sum, entry) => sum + entry.badges.length,
    0,
  );

  return (
    <>
      <p className="my-4">
        Badge report for this location. Use this to spot repeat winners, recent
        awards, and members who are building momentum.
      </p>
      <p className="my-4">
        <TextLink to="/admin/members">Back to members</TextLink>
      </p>

      <div className="my-4 grid grid-cols-[repeat(auto-fit,minmax(220px,1fr))] gap-3">
        <article className="rounded-xl border border-[#d7c3b1] bg-linear-to-b from-[#fff9f1] to-white px-4 py-3.5">
          <div className="mb-1 text-sm tracking-[0.06em] text-badge-accent uppercase">
            Members with badges
          </div>
          <div className="font-title text-2xl text-navy">{people.length}</div>
        </article>
        <article className="rounded-xl border border-[#d7c3b1] bg-linear-to-b from-[#fff9f1] to-white px-4 py-3.5">
          <div className="mb-1 text-sm tracking-[0.06em] text-badge-accent uppercase">
            Total badge awards
          </div>
          <div className="font-title text-2xl text-navy">{totalAwards}</div>
        </article>
      </div>

      <TrophyCabinet
        trophies={location.trophies}
        unitName={selectedLocation.name}
      />

      <SectionHeading>Members' badges</SectionHeading>
      <AdminTable>
        <thead>
          <tr>
            <Th style={{ width: 56 }}>#</Th>
            <Th>Name</Th>
            <Th style={{ width: 110 }}>SES ID</Th>
            <Th style={{ width: 120 }}>Badges</Th>
            <Th style={{ width: 140 }}>Recent 7d</Th>
            <Th>Latest award</Th>
          </tr>
        </thead>
        <tbody>
          {people.length === 0 ? (
            <tr>
              <Td colSpan={6} className="px-1.5 py-4.5 text-ink-muted">
                No visible badges have been earned at this location yet.
              </Td>
            </tr>
          ) : (
            people.map(({ person, badges, recentCount }, idx) => (
              <tr
                key={person.id}
                className={idx % 2 === 0 ? "bg-surface-raised" : undefined}
              >
                <Td>{ranks[idx]}</Td>
                <Td nowrap>
                  <TextLink to={`/admin/members/badges/${person.id}`}>
                    {person.firstName} {person.lastName}
                  </TextLink>
                </Td>
                <Td>{person.memberNumber || "-"}</Td>
                <Td>
                  <span className="inline-block min-w-6.5 rounded-full bg-badge-accent px-2 py-0.5 text-center font-bold text-white">
                    {badges.length}
                  </span>
                </Td>
                <Td>
                  <span className="inline-block min-w-6.5 rounded-full bg-navy px-2 py-0.5 text-center font-bold text-white">
                    {recentCount}
                  </span>
                </Td>
                <Td className="whitespace-normal">
                  {formatFullDateTime(new Date(badges[0].awardedAt * 1000))}
                  <div className="mt-0.5 text-sm text-ink-muted">
                    {badges[0].name}
                  </div>
                </Td>
              </tr>
            ))
          )}
        </tbody>
      </AdminTable>
    </>
  );
}

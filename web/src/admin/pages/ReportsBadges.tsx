import { graphql, useLazyLoadQuery } from "react-relay";
import { Link } from "react-router";
import useSelectedLocation from "../components/useSelectedLocation";
import { formatFullDateTime } from "../../lib/time";
import type { ReportsBadgesQuery } from "./__generated__/ReportsBadgesQuery.graphql";
import { AdminTable, Th, Td } from "../../components/ui/Table";

type ReportPerson = NonNullable<
  NonNullable<ReportsBadgesQuery["response"]>["location"]
>["people"][number];

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
  const data = useLazyLoadQuery<ReportsBadgesQuery>(
    graphql`
      query ReportsBadgesQuery($location: ID!) {
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
        }
      }
    `,
    { location: locationId },
    { fetchKey: locationId },
  );

  if (!selectedLocation.gamificationEnabled) {
    return (
      <>
        <p>Gamification is disabled for this location.</p>
        <p>
          <Link to="/admin/reports">Back to reports</Link>
        </p>
      </>
    );
  }

  const location = data?.location;
  const people = [...location.people]
    .filter((person): person is NonNullable<typeof person> => person != null)
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

  const totalAwards = people.reduce(
    (sum, entry) => sum + entry.badges.length,
    0,
  );

  return (
    <>
      <p>
        Badge report for this location. Use this to spot repeat winners, recent
        awards, and members who are building momentum.
      </p>
      <p>
        <Link to="/admin/members">Back to members</Link>
      </p>

      <div className="my-4 grid grid-cols-[repeat(auto-fit,minmax(220px,1fr))] gap-3">
        <article className="rounded-xl border border-[#d7c3b1] bg-linear-to-b from-[#fff9f1] to-white px-4 py-3.5">
          <div className="mb-1 text-[0.85rem] tracking-[0.06em] text-[#7f3f11] uppercase">
            Members with badges
          </div>
          <div className="font-title text-2xl text-navy">{people.length}</div>
        </article>
        <article className="rounded-xl border border-[#d7c3b1] bg-linear-to-b from-[#fff9f1] to-white px-4 py-3.5">
          <div className="mb-1 text-[0.85rem] tracking-[0.06em] text-[#7f3f11] uppercase">
            Total badge awards
          </div>
          <div className="font-title text-2xl text-navy">{totalAwards}</div>
        </article>
      </div>

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
              <Td colSpan={6} className="px-1.5 py-4.5 text-[#555555]">
                No visible badges have been earned at this location yet.
              </Td>
            </tr>
          ) : (
            people.map(({ person, badges, recentCount }, idx) => (
              <tr
                key={person.id}
                className={idx % 2 === 0 ? "bg-neutral-50" : undefined}
              >
                <Td>{idx + 1}</Td>
                <Td nowrap>
                  {person.firstName} {person.lastName}
                </Td>
                <Td>{person.memberNumber || "-"}</Td>
                <Td>
                  <span className="inline-block min-w-6.5 rounded-full bg-[#7f3f11] px-2 py-0.5 text-center font-bold text-white">
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
                  <div className="mt-0.5 text-[0.88rem] text-[#5b5b5b]">
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

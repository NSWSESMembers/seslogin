import { graphql, useLazyLoadQuery } from "react-relay";
import { Link } from "react-router";
import useSelectedLocation from "../components/useSelectedLocation";
import { formatFullDateTime } from "../../lib/time";
import type { ReportsBadgesQuery } from "./__generated__/ReportsBadgesQuery.graphql";

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

      <div className="badge-report-summary-grid">
        <article className="badge-report-summary-card">
          <div className="badge-report-summary-label">Members with badges</div>
          <div className="badge-report-summary-value">{people.length}</div>
        </article>
        <article className="badge-report-summary-card">
          <div className="badge-report-summary-label">Total badge awards</div>
          <div className="badge-report-summary-value">{totalAwards}</div>
        </article>
      </div>

      <table className="admin badge-report-table">
        <thead>
          <tr>
            <th style={{ width: 56 }}>#</th>
            <th>Name</th>
            <th style={{ width: 110 }}>SES ID</th>
            <th style={{ width: 120 }}>Badges</th>
            <th style={{ width: 140 }}>Recent 7d</th>
            <th>Latest award</th>
          </tr>
        </thead>
        <tbody>
          {people.length === 0 ? (
            <tr>
              <td colSpan={6} className="badge-report-empty">
                No visible badges have been earned at this location yet.
              </td>
            </tr>
          ) : (
            people.map(({ person, badges, recentCount }, idx) => (
              <tr key={person.id} className={idx % 2 === 0 ? "odd" : "even"}>
                <td>{idx + 1}</td>
                <td className="nowrap">
                  {person.firstName} {person.lastName}
                </td>
                <td>{person.memberNumber || "-"}</td>
                <td>
                  <span className="badge-report-count">{badges.length}</span>
                </td>
                <td>
                  <span className="badge-report-recent">{recentCount}</span>
                </td>
                <td className="badge-report-latest">
                  {formatFullDateTime(new Date(badges[0].awardedAt * 1000))}
                  <div className="badge-report-latest-name">
                    {badges[0].name}
                  </div>
                </td>
              </tr>
            ))
          )}
        </tbody>
      </table>
    </>
  );
}

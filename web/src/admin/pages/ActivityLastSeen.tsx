import { graphql, useLazyLoadQuery } from "react-relay";
import { useState } from "react";
import { useSettings } from "../../lib/settings";
import { formatFullDateTime, formatTimeDiff } from "../../lib/time";
import type { ActivityLastSeenQuery } from "./__generated__/ActivityLastSeenQuery.graphql";

type SortMode = "alphabetical" | "recent" | "least-recent";

export default function ActivityLastSeen() {
  const settings = useSettings();
  const [sort, setSort] = useState<SortMode>("recent");

  const data = useLazyLoadQuery<ActivityLastSeenQuery>(
    graphql`
      query ActivityLastSeenQuery($location: ID!) {
        location(id: $location) {
          id
          people {
            id
            firstName
            lastName
            lastPeriod {
              startTime
              endTime
            }
          }
        }
      }
    `,
    { location: settings?.locationId || "" },
  );

  const members = data.location.people.map((person) => {
    const lastSeen = person.lastPeriod
      ? (person.lastPeriod.endTime ?? person.lastPeriod.startTime)
      : null;
    return { ...person, lastSeen };
  });

  const sorted = [...members].sort((a, b) => {
    if (sort === "alphabetical") {
      return (
        a.lastName.localeCompare(b.lastName) ||
        a.firstName.localeCompare(b.firstName)
      );
    }
    // Never-seen members always sort to the bottom.
    if (a.lastSeen === null && b.lastSeen === null) return 0;
    if (a.lastSeen === null) return 1;
    if (b.lastSeen === null) return -1;
    return sort === "recent"
      ? b.lastSeen - a.lastSeen
      : a.lastSeen - b.lastSeen;
  });

  const now = new Date();

  return (
    <>
      <div style={{ marginBottom: "1em" }}>
        <label>
          Sort:{" "}
          <select
            value={sort}
            onChange={(e) => setSort(e.target.value as SortMode)}
          >
            <option value="recent">Most recent</option>
            <option value="least-recent">Least recent</option>
            <option value="alphabetical">Alphabetical</option>
          </select>
        </label>
      </div>
      <table className="admin">
        <thead>
          <tr>
            <th>Name</th>
            <th>Last seen</th>
            <th>Ago</th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((member, idx) => {
            const lastSeenDate =
              member.lastSeen !== null
                ? new Date(member.lastSeen * 1000)
                : null;
            return (
              <tr key={member.id} className={idx % 2 === 0 ? "odd" : "even"}>
                <td data-label="Name">
                  {member.firstName} {member.lastName}
                </td>
                <td data-label="Last Seen">
                  {lastSeenDate ? formatFullDateTime(lastSeenDate) : "Never"}
                </td>
                <td data-label="Time Ago">
                  {lastSeenDate
                    ? formatTimeDiff(lastSeenDate, now) + " ago"
                    : "-"}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </>
  );
}

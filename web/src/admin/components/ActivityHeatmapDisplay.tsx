import { graphql } from "relay-runtime";
import { useLazyLoadQuery } from "react-relay";
import ActivityHeatmapGrid from "./ActivityHeatmapGrid";
import {
  buildHeatmapColumns,
  buildRowCells,
  sortHeatmapRows,
  type HeatmapCell,
  type HeatmapScale,
  type HeatmapSortBy,
} from "./activityHeatmapBuckets";
import type { ActivityHeatmapDisplayQuery } from "./__generated__/ActivityHeatmapDisplayQuery.graphql";

interface ActivityHeatmapDisplayProps {
  locationId: string;
  startTime: number;
  endTime: number;
  scale: HeatmapScale;
  categoryId: string;
  sortBy: HeatmapSortBy;
}

export default function ActivityHeatmapDisplay({
  locationId,
  startTime,
  endTime,
  scale,
  categoryId,
  sortBy,
}: ActivityHeatmapDisplayProps) {
  const data = useLazyLoadQuery<ActivityHeatmapDisplayQuery>(
    graphql`
      query ActivityHeatmapDisplayQuery(
        $location: ID!
        $startTime: Int!
        $endTime: Int!
        $category: ID
      ) {
        location(id: $location) {
          id
          people {
            id
            firstName
            lastName
          }
          periodSummaryByDayByMember(
            startTime: $startTime
            endTime: $endTime
            category: $category
          ) {
            date
            members {
              person {
                id
              }
              totalTime
              periodCount
            }
          }
        }
      }
    `,
    {
      location: locationId,
      startTime,
      endTime,
      category: categoryId || null,
    },
  );

  // Sparse (personId, dayKey) -> cell map from the query's day-by-day rows.
  const cellsByPerson = new Map<string, Map<string, HeatmapCell>>();
  for (const day of data.location.periodSummaryByDayByMember) {
    for (const member of day.members) {
      let byDay = cellsByPerson.get(member.person.id);
      if (!byDay) {
        byDay = new Map();
        cellsByPerson.set(member.person.id, byDay);
      }
      byDay.set(day.date, {
        totalTime: member.totalTime,
        periodCount: member.periodCount,
      });
    }
  }

  // Full column axis for the whole range (not just columns with activity),
  // so a day/week/month with zero activity for everyone still renders as an
  // empty column — that's what makes a location-wide gap visible.
  const columns = buildHeatmapColumns(startTime, endTime, scale);

  const unsortedRows = data.location.people.map((person) => {
    const cells = buildRowCells(cellsByPerson.get(person.id), columns);
    return {
      personId: person.id,
      name: `${person.firstName} ${person.lastName}`.trim(),
      cells,
    };
  });
  const rows = sortHeatmapRows(unsortedRows, sortBy);

  // buildHeatmapColumns/buildRowCells/sortHeatmapRows all operate in
  // ascending chronological order (e.g. "most recently active" means the
  // highest cell index) — reverse only for display, so the grid reads most
  // recent first (left) to oldest (right).
  const displayColumns = [...columns].reverse();
  const displayRows = rows.map((row) => ({
    ...row,
    cells: [...row.cells].reverse(),
  }));

  return <ActivityHeatmapGrid columns={displayColumns} rows={displayRows} />;
}

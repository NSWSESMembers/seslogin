import { graphql } from "relay-runtime";
import { useLazyLoadQuery } from "react-relay";
import ActivityTotalsTable, {
  type ActivityTotalsRow,
} from "./ActivityTotalsTable";
import type { ActivityTotalsDisplayQuery } from "./__generated__/ActivityTotalsDisplayQuery.graphql";

interface ActivityTotalsDisplayProps {
  locationId: string;
  startTime: number;
  endTime: number;
  category?: string;
}

export default function ActivityTotalsDisplay({
  locationId,
  startTime,
  endTime,
  category,
}: ActivityTotalsDisplayProps) {
  const data = useLazyLoadQuery<ActivityTotalsDisplayQuery>(
    graphql`
      query ActivityTotalsDisplayQuery(
        $location: ID!
        $startTime: Int!
        $endTime: Int!
        $category: ID
      ) {
        location(id: $location) {
          id
          periodSummaryByMember(
            startTime: $startTime
            endTime: $endTime
            category: $category
          ) {
            person {
              id
              firstName
              lastName
            }
            totalTime
          }
          periodSummaryByCategory(startTime: $startTime, endTime: $endTime) {
            category {
              id
              name
            }
            totalTime
          }
        }
      }
    `,
    {
      location: locationId,
      startTime,
      endTime,
      category: category || null,
    },
  );

  const memberRows: ReadonlyArray<ActivityTotalsRow> =
    data.location.periodSummaryByMember.map((entry) => ({
      id: entry.person.id,
      name: `${entry.person.firstName} ${entry.person.lastName}`,
      totalTime: entry.totalTime,
    }));

  const categoryRows: ReadonlyArray<ActivityTotalsRow> =
    data.location.periodSummaryByCategory.map((entry) => ({
      id: entry.category.id,
      name: entry.category.name,
      totalTime: entry.totalTime,
    }));

  return (
    <div className="flex items-start gap-5 max-md:flex-col">
      <ActivityTotalsTable title="Time per member" rows={memberRows} />
      {!category && (
        <ActivityTotalsTable title="Time per category" rows={categoryRows} />
      )}
    </div>
  );
}

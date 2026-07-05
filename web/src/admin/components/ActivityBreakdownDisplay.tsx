import { graphql } from "relay-runtime";
import { useLazyLoadQuery } from "react-relay";
import ActivityBreakdownTable, {
  type ActivityBreakdownGroupRow,
} from "./ActivityBreakdownTable";
import type { ActivityBreakdownDisplayQuery } from "./__generated__/ActivityBreakdownDisplayQuery.graphql";

interface ActivityBreakdownDisplayProps {
  locationId: string;
  startTime: number;
  endTime: number;
}

export default function ActivityBreakdownDisplay({
  locationId,
  startTime,
  endTime,
}: ActivityBreakdownDisplayProps) {
  const data = useLazyLoadQuery<ActivityBreakdownDisplayQuery>(
    graphql`
      query ActivityBreakdownDisplayQuery(
        $location: ID!
        $startTime: Int!
        $endTime: Int!
      ) {
        location(id: $location) {
          id
          periodSummaryByMemberByCategory(
            startTime: $startTime
            endTime: $endTime
          ) {
            person {
              id
              firstName
              lastName
            }
            totalTime
            categories {
              category {
                id
                name
              }
              totalTime
            }
          }
          periodSummaryByCategoryByMember(
            startTime: $startTime
            endTime: $endTime
          ) {
            category {
              id
              name
            }
            totalTime
            members {
              person {
                id
                firstName
                lastName
              }
              totalTime
            }
          }
        }
      }
    `,
    {
      location: locationId,
      startTime,
      endTime,
    },
  );

  const memberCategoryRows: ReadonlyArray<ActivityBreakdownGroupRow> =
    data.location.periodSummaryByMemberByCategory.map((entry) => ({
      id: entry.person.id,
      name: `${entry.person.firstName} ${entry.person.lastName}`,
      totalTime: entry.totalTime,
      children: entry.categories.map((category) => ({
        id: category.category.id,
        name: category.category.name,
        totalTime: category.totalTime,
      })),
    }));

  const categoryMemberRows: ReadonlyArray<ActivityBreakdownGroupRow> =
    data.location.periodSummaryByCategoryByMember.map((entry) => ({
      id: entry.category.id,
      name: entry.category.name,
      totalTime: entry.totalTime,
      children: entry.members.map((member) => ({
        id: member.person.id,
        name: `${member.person.firstName} ${member.person.lastName}`,
        totalTime: member.totalTime,
      })),
    }));

  return (
    <div className="flex items-start gap-5 max-md:flex-col">
      <ActivityBreakdownTable
        title="Time per member per category"
        rows={memberCategoryRows}
      />
      <ActivityBreakdownTable
        title="Time per category per member"
        rows={categoryMemberRows}
      />
    </div>
  );
}

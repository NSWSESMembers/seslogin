import { graphql } from "relay-runtime";
import { useLazyLoadQuery } from "react-relay";
import ActivityDailyBreakdownTable, {
  type ActivityDailyBreakdownDayRow,
} from "./ActivityDailyBreakdownTable";
import type { ActivityDailyBreakdownDisplayQuery } from "./__generated__/ActivityDailyBreakdownDisplayQuery.graphql";

interface ActivityDailyBreakdownDisplayProps {
  locationId: string;
  startTime: number;
  endTime: number;
}

export default function ActivityDailyBreakdownDisplay({
  locationId,
  startTime,
  endTime,
}: ActivityDailyBreakdownDisplayProps) {
  const data = useLazyLoadQuery<ActivityDailyBreakdownDisplayQuery>(
    graphql`
      query ActivityDailyBreakdownDisplayQuery(
        $location: ID!
        $startTime: Int!
        $endTime: Int!
      ) {
        location(id: $location) {
          id
          periodSummaryByDayByCategoryByMember(
            startTime: $startTime
            endTime: $endTime
          ) {
            date
            totalTime
            categories {
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
      }
    `,
    {
      location: locationId,
      startTime,
      endTime,
    },
  );

  const days: ReadonlyArray<ActivityDailyBreakdownDayRow> =
    data.location.periodSummaryByDayByCategoryByMember.map((day) => ({
      date: day.date,
      totalTime: day.totalTime,
      categories: day.categories.map((category) => ({
        id: category.category.id,
        name: category.category.name,
        totalTime: category.totalTime,
        members: category.members.map((member) => ({
          id: member.person.id,
          name: `${member.person.firstName} ${member.person.lastName}`,
          totalTime: member.totalTime,
        })),
      })),
    }));

  return <ActivityDailyBreakdownTable days={days} />;
}

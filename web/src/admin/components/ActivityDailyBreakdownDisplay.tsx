import { graphql } from "relay-runtime";
import { useLazyLoadQuery } from "react-relay";
import ActivityDailyBreakdownTable, {
  type ActivityDailyBreakdownDayRow,
} from "./ActivityDailyBreakdownTable";
import type { ActivityDailyBreakdownDisplayQuery } from "./__generated__/ActivityDailyBreakdownDisplayQuery.graphql";
import { useUserInfo } from "./useUserInfo";
import { formatSeconds } from "../../lib/time";

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
  const { disaggregateVirtualPeriods } = useUserInfo();

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
                isVirtual
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
    data.location.periodSummaryByDayByCategoryByMember.map((day) => {
      const virtualTime = day.categories
        .filter((c) => c.category.isVirtual)
        .reduce((sum, c) => sum + c.totalTime, 0);
      const nonVirtualTime = day.totalTime - virtualTime;
      return {
        date: day.date,
        totalTime: day.totalTime,
        splitLine: disaggregateVirtualPeriods
          ? `${formatSeconds(virtualTime)} virtual · ${formatSeconds(nonVirtualTime)} non-virtual`
          : undefined,
        categories: day.categories.map((category) => ({
          id: category.category.id,
          name: category.category.name,
          totalTime: category.totalTime,
          isVirtual: disaggregateVirtualPeriods
            ? category.category.isVirtual
            : undefined,
          members: category.members.map((member) => ({
            id: member.person.id,
            name: `${member.person.firstName} ${member.person.lastName}`,
            totalTime: member.totalTime,
          })),
        })),
      };
    });

  return <ActivityDailyBreakdownTable days={days} />;
}

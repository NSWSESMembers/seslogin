import { useState } from "react";
import { graphql } from "relay-runtime";
import { useLazyLoadQuery } from "react-relay";
import ActivityDailyBreakdownTable, {
  type ActivityDailyBreakdownDayRow,
} from "./ActivityDailyBreakdownTable";
import type { ActivityDailyBreakdownDisplayQuery } from "./__generated__/ActivityDailyBreakdownDisplayQuery.graphql";
import { useUserInfo } from "./useUserInfo";
import { formatSeconds } from "../../lib/time";
import { useRelayRetryFetchKey } from "../../components/relayRetryContext";

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
  const [hideVirtual, setHideVirtual] = useState(false);
  const showSplit = disaggregateVirtualPeriods && !hideVirtual;
  const fetchKey = useRelayRetryFetchKey();

  const data = useLazyLoadQuery<ActivityDailyBreakdownDisplayQuery>(
    graphql`
      query ActivityDailyBreakdownDisplayQuery(
        $location: ID!
        $startTime: Int!
        $endTime: Int!
      ) @throwOnFieldError {
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
    { fetchKey },
  );

  const days: ReadonlyArray<ActivityDailyBreakdownDayRow> =
    data.location.periodSummaryByDayByCategoryByMember
      .map((day) => {
        const virtualTime = day.categories
          .filter((c) => c.category.isVirtual)
          .reduce((sum, c) => sum + c.totalTime, 0);
        const nonVirtualTime = day.totalTime - virtualTime;
        const categories = hideVirtual
          ? day.categories.filter((c) => !c.category.isVirtual)
          : day.categories;
        return {
          date: day.date,
          totalTime: hideVirtual ? nonVirtualTime : day.totalTime,
          splitLine: showSplit
            ? `${formatSeconds(virtualTime)} virtual · ${formatSeconds(nonVirtualTime)} non-virtual`
            : undefined,
          categories: categories.map((category) => ({
            id: category.category.id,
            name: category.category.name,
            totalTime: category.totalTime,
            isVirtual: showSplit ? category.category.isVirtual : undefined,
            members: category.members.map((member) => ({
              id: member.person.id,
              name: `${member.person.firstName} ${member.person.lastName}`,
              totalTime: member.totalTime,
            })),
          })),
        };
      })
      .filter((day) => day.categories.length > 0);

  return (
    <>
      {disaggregateVirtualPeriods && (
        <label className="mb-3 flex items-center justify-end gap-2">
          <input
            type="checkbox"
            checked={hideVirtual}
            onChange={(e) => setHideVirtual(e.target.checked)}
          />
          Hide virtual
        </label>
      )}
      <ActivityDailyBreakdownTable days={days} />
    </>
  );
}

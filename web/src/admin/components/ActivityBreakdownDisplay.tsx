import { graphql } from "relay-runtime";
import { useLazyLoadQuery } from "react-relay";
import ActivityBreakdownTable, {
  type ActivityBreakdownGroupRow,
} from "./ActivityBreakdownTable";
import type { ActivityBreakdownDisplayQuery } from "./__generated__/ActivityBreakdownDisplayQuery.graphql";
import { useUserInfo } from "./useUserInfo";
import { formatSeconds } from "../../lib/time";

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
  const { disaggregateVirtualPeriods } = useUserInfo();

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
                isVirtual
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
    `,
    {
      location: locationId,
      startTime,
      endTime,
    },
  );

  const memberCategoryRows: ReadonlyArray<ActivityBreakdownGroupRow> =
    data.location.periodSummaryByMemberByCategory.map((entry) => {
      const virtualTime = entry.categories
        .filter((c) => c.category.isVirtual)
        .reduce((sum, c) => sum + c.totalTime, 0);
      const nonVirtualTime = entry.totalTime - virtualTime;
      return {
        id: entry.person.id,
        name: `${entry.person.firstName} ${entry.person.lastName}`,
        totalTime: entry.totalTime,
        splitLine: disaggregateVirtualPeriods
          ? `${formatSeconds(virtualTime)} virtual · ${formatSeconds(nonVirtualTime)} non-virtual`
          : undefined,
        children: entry.categories.map((category) => ({
          id: category.category.id,
          name: category.category.name,
          totalTime: category.totalTime,
          isVirtual: disaggregateVirtualPeriods
            ? category.category.isVirtual
            : undefined,
        })),
      };
    });

  function toCategoryMemberRow(
    entry: (typeof data.location.periodSummaryByCategoryByMember)[number],
  ): ActivityBreakdownGroupRow {
    return {
      id: entry.category.id,
      name: entry.category.name,
      totalTime: entry.totalTime,
      children: entry.members.map((member) => ({
        id: member.person.id,
        name: `${member.person.firstName} ${member.person.lastName}`,
        totalTime: member.totalTime,
      })),
    };
  }
  const categoryMemberRows: ReadonlyArray<ActivityBreakdownGroupRow> =
    data.location.periodSummaryByCategoryByMember.map(toCategoryMemberRow);
  const virtualCategoryMemberRows: ReadonlyArray<ActivityBreakdownGroupRow> =
    data.location.periodSummaryByCategoryByMember
      .filter((entry) => entry.category.isVirtual)
      .map(toCategoryMemberRow);
  const nonVirtualCategoryMemberRows: ReadonlyArray<ActivityBreakdownGroupRow> =
    data.location.periodSummaryByCategoryByMember
      .filter((entry) => !entry.category.isVirtual)
      .map(toCategoryMemberRow);

  return (
    <div className="flex items-start gap-5 max-md:flex-col">
      <ActivityBreakdownTable
        title="Time per member per category"
        rows={memberCategoryRows}
      />
      {disaggregateVirtualPeriods ? (
        <>
          <ActivityBreakdownTable
            title="Time per category per member — Virtual"
            rows={virtualCategoryMemberRows}
          />
          <ActivityBreakdownTable
            title="Time per category per member — Non-virtual"
            rows={nonVirtualCategoryMemberRows}
          />
        </>
      ) : (
        <ActivityBreakdownTable
          title="Time per category per member"
          rows={categoryMemberRows}
        />
      )}
    </div>
  );
}

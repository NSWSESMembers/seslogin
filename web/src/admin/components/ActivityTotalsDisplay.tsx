import { useState } from "react";
import { graphql } from "relay-runtime";
import { useLazyLoadQuery } from "react-relay";
import ActivityTotalsTable, {
  type ActivityTotalsRow,
} from "./ActivityTotalsTable";
import type { ActivityTotalsDisplayQuery } from "./__generated__/ActivityTotalsDisplayQuery.graphql";
import { useUserInfo } from "./useUserInfo";
import { useRelayRetryFetchKey } from "../../components/relayRetryContext";

interface ActivityTotalsDisplayProps {
  locationId: string;
  startTime: number;
  endTime: number;
  categories: ReadonlyArray<string>;
}

export default function ActivityTotalsDisplay({
  locationId,
  startTime,
  endTime,
  categories,
}: ActivityTotalsDisplayProps) {
  const { disaggregateVirtualPeriods } = useUserInfo();
  const [hideVirtual, setHideVirtual] = useState(false);
  const showSplit = disaggregateVirtualPeriods && !hideVirtual;
  const fetchKey = useRelayRetryFetchKey();

  const data = useLazyLoadQuery<ActivityTotalsDisplayQuery>(
    graphql`
      query ActivityTotalsDisplayQuery(
        $location: ID!
        $startTime: Int!
        $endTime: Int!
        $categories: [ID!]
      ) @throwOnFieldError {
        location(id: $location) {
          id
          periodSummaryByMember(
            startTime: $startTime
            endTime: $endTime
            categories: $categories
          ) {
            person {
              id
              firstName
              lastName
            }
            totalTime
            totalTimeVirtual
          }
          periodSummaryByCategory(startTime: $startTime, endTime: $endTime) {
            category {
              id
              name
              isVirtual
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
      categories: categories.length > 0 ? categories : null,
    },
    { fetchKey },
  );

  const memberRows: ReadonlyArray<ActivityTotalsRow> =
    data.location.periodSummaryByMember.map((entry) => {
      // The API sends only the virtual portion; non-virtual is the remainder.
      const totalTimeVirtual = entry.totalTimeVirtual ?? 0;
      const totalTimeNonVirtual = entry.totalTime - totalTimeVirtual;
      return {
        id: entry.person.id,
        name: `${entry.person.firstName} ${entry.person.lastName}`,
        totalTime: hideVirtual ? totalTimeNonVirtual : entry.totalTime,
        totalTimeVirtual,
        totalTimeNonVirtual,
      };
    });

  const categoryRows: ReadonlyArray<ActivityTotalsRow> =
    data.location.periodSummaryByCategory.map((entry) => ({
      id: entry.category.id,
      name: entry.category.name,
      totalTime: entry.totalTime,
    }));
  const virtualCategoryRows: ReadonlyArray<ActivityTotalsRow> =
    data.location.periodSummaryByCategory
      .filter((entry) => entry.category.isVirtual)
      .map((entry) => ({
        id: entry.category.id,
        name: entry.category.name,
        totalTime: entry.totalTime,
      }));
  const nonVirtualCategoryRows: ReadonlyArray<ActivityTotalsRow> =
    data.location.periodSummaryByCategory
      .filter((entry) => !entry.category.isVirtual)
      .map((entry) => ({
        id: entry.category.id,
        name: entry.category.name,
        totalTime: entry.totalTime,
      }));

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
      <div className="flex items-start gap-5 max-md:flex-col">
        <ActivityTotalsTable
          title="Time per member"
          rows={memberRows}
          showSplit={showSplit}
        />
        {categories.length === 0 &&
          (showSplit ? (
            <>
              <ActivityTotalsTable
                title="Time per category — Virtual"
                rows={virtualCategoryRows}
              />
              <ActivityTotalsTable
                title="Time per category — Non-virtual"
                rows={nonVirtualCategoryRows}
              />
            </>
          ) : (
            <ActivityTotalsTable
              title="Time per category"
              rows={
                disaggregateVirtualPeriods
                  ? nonVirtualCategoryRows
                  : categoryRows
              }
            />
          ))}
      </div>
    </>
  );
}

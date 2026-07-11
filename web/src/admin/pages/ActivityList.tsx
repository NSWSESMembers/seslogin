import { graphql, readInlineData } from "relay-runtime";
import { fetchQuery, useLazyLoadQuery, useRelayEnvironment } from "react-relay";
import { useSettings } from "../../lib/settings";
import { startTransition, useEffect, useState } from "react";
import type {
  ActivityListQuery,
  ActivityListQuery$data,
} from "./__generated__/ActivityListQuery.graphql";
import type { ActivityList_periodName$key } from "./__generated__/ActivityList_periodName.graphql";
import ActivityListTable from "../components/ActivityListTable";

const ACTIVITY_PAGE_SIZE = 100;

type PeriodRef = NonNullable<
  NonNullable<
    ActivityListQuery$data["location"]["periods"]["edges"][number]
  >["node"]
>;

// The display name for this (per-location) view is the member's name. Colocate
// that data dependency here, read inside getRowLabel from the same period ref.
const activityListPeriodName = graphql`
  fragment ActivityList_periodName on Period @inline {
    guestName
    person {
      id
      firstName
      lastName
    }
  }
`;

export default function ActivityList() {
  const settings = useSettings();
  const relayEnvironment = useRelayEnvironment();
  const data = useLazyLoadQuery<ActivityListQuery>(
    graphql`
      query ActivityListQuery($location: ID!, $first: Int!, $after: String) {
        location(id: $location) {
          id
          periods(first: $first, after: $after) {
            edges {
              node {
                ...ActivityListTable_period
                ...ActivityList_periodName
              }
            }
            pageInfo {
              hasNextPage
              endCursor
            }
          }
        }
      }
    `,
    {
      location: settings?.locationId || "",
      first: ACTIVITY_PAGE_SIZE,
      after: null,
    },
  );

  const [periods, setPeriods] = useState<PeriodRef[]>([]);
  const [hasNextPage, setHasNextPage] = useState(false);
  const [endCursor, setEndCursor] = useState<string | null>(null);
  const [isLoadingMore, setIsLoadingMore] = useState(false);

  useEffect(() => {
    const nextPeriods = data.location.periods.edges
      // edges can be null if we do client side deletes from the relay store
      .filter((edge): edge is NonNullable<typeof edge> => edge.node !== null)
      .map((edge) => edge.node);
    startTransition(() => {
      setPeriods(nextPeriods);
      setHasNextPage(data.location.periods.pageInfo.hasNextPage);
      setEndCursor(data.location.periods.pageInfo.endCursor ?? null);
    });
  }, [
    data.location.id,
    data.location.periods.edges,
    data.location.periods.pageInfo,
  ]);

  function getRowLabel(periodRef: PeriodRef) {
    const { person, guestName } = readInlineData<ActivityList_periodName$key>(
      activityListPeriodName,
      periodRef,
    );
    return person
      ? `${person.firstName} ${person.lastName}`
      : `${guestName ?? "Guest"} (Guest)`;
  }

  async function onLoadMore() {
    if (!hasNextPage || !endCursor || isLoadingMore) {
      return;
    }

    setIsLoadingMore(true);
    try {
      const next = await fetchQuery<ActivityListQuery>(
        relayEnvironment,
        graphql`
          query ActivityListLoadMoreQuery(
            $location: ID!
            $first: Int!
            $after: String
          ) {
            location(id: $location) {
              id
              periods(first: $first, after: $after) {
                edges {
                  node {
                    ...ActivityListTable_period
                    ...ActivityList_periodName
                  }
                }
                pageInfo {
                  hasNextPage
                  endCursor
                }
              }
            }
          }
        `,
        {
          location: settings?.locationId || "",
          first: ACTIVITY_PAGE_SIZE,
          after: endCursor,
        },
      ).toPromise();

      const nextPeriods =
        next?.location.periods.edges.map((edge) => edge.node) ?? [];
      setPeriods((previous) => [...previous, ...nextPeriods]);
      setHasNextPage(next?.location.periods.pageInfo.hasNextPage ?? false);
      setEndCursor(next?.location.periods.pageInfo.endCursor ?? null);
    } finally {
      setIsLoadingMore(false);
    }
  }

  return (
    <ActivityListTable
      firstcol="person"
      periods={periods}
      getRowLabel={getRowLabel}
      hasNextPage={hasNextPage}
      isLoadingMore={isLoadingMore}
      onLoadMore={onLoadMore}
    />
  );
}

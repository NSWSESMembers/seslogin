import { graphql, readInlineData } from "relay-runtime";
import { fetchQuery, useRelayEnvironment } from "react-relay";
import { useSettings } from "../../lib/settings";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import { unwrapCatch } from "../../lib/relayCatch";
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
    # @catch so one dangling member reference degrades that row instead of
    # (via @throwOnFieldError on the enclosing query) hiding the whole page.
    # Also lets getRowLabel tell "no person, has a guest name" apart from
    # "person lookup failed" — a real member must never render as a guest.
    person @catch {
      id
      firstName
      lastName
    }
  }
`;

export default function ActivityList() {
  const settings = useSettings();
  const relayEnvironment = useRelayEnvironment();
  const data = useRetryableLazyLoadQuery<ActivityListQuery>(
    graphql`
      query ActivityListQuery($location: ID!, $first: Int!, $after: String)
      @throwOnFieldError {
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
  const [loadMoreError, setLoadMoreError] = useState<string | null>(null);

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
    // Throwing unwrap: a failed lookup here is caught by the per-row
    // ErrorBoundary this is always called from within (see ActivityListTable),
    // degrading just that row instead of misattributing activity to a guest.
    const personValue = unwrapCatch(person);
    return personValue
      ? `${personValue.firstName} ${personValue.lastName}`
      : `${guestName ?? "Guest"} (Guest)`;
  }

  async function onLoadMore() {
    if (!hasNextPage || !endCursor || isLoadingMore) {
      return;
    }

    setIsLoadingMore(true);
    setLoadMoreError(null);
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
        next?.location.periods.edges
          // Defensive, same as the initial load: a null edge/node shouldn't be
          // possible per the schema, but a locally mutated store can produce one.
          .filter(
            (edge): edge is NonNullable<typeof edge> => edge?.node != null,
          )
          .map((edge) => edge.node) ?? [];
      setPeriods((previous) => [...previous, ...nextPeriods]);
      setHasNextPage(next?.location.periods.pageInfo.hasNextPage ?? false);
      setEndCursor(next?.location.periods.pageInfo.endCursor ?? null);
    } catch (err) {
      console.error("Failed to load more activity:", err);
      setLoadMoreError("Couldn't load more — please try again.");
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
      loadMoreError={loadMoreError}
    />
  );
}

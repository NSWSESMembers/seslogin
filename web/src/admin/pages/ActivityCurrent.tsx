import { graphql, isValueResult, readInlineData } from "relay-runtime";
import { fetchQuery, useRelayEnvironment } from "react-relay";
import { useSettings } from "../../lib/settings";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import { unwrapCatch } from "../../lib/relayCatch";
import { Suspense, startTransition, useEffect, useState } from "react";
import type {
  ActivityCurrentQuery,
  ActivityCurrentQuery$data,
} from "./__generated__/ActivityCurrentQuery.graphql";
import type { ActivityCurrent_periodName$key } from "./__generated__/ActivityCurrent_periodName.graphql";
import ActivityListTable from "../components/ActivityListTable";
import ActivityCategorySelector from "../components/ActivityCategorySelector";
import LoadingIndicator from "../../components/LoadingIndicator";
import TextInput from "../../components/ui/TextInput";

const ACTIVITY_CURRENT_PAGE_SIZE = 100;

type PeriodRef = NonNullable<
  NonNullable<
    ActivityCurrentQuery$data["location"]["periods"]["edges"][number]
  >["node"]
>;

// The display name for this (per-location) view is the member's name. Colocate
// that data dependency here, read inside getRowLabel from the same period ref.
// memberNumber is only used for the text filter below, never displayed.
const activityCurrentPeriodName = graphql`
  fragment ActivityCurrent_periodName on Period @inline {
    guestName
    # @catch so one dangling member reference degrades that row instead of
    # (via @throwOnFieldError on the enclosing query) hiding the whole page.
    # Also lets getRowLabel tell "no person, has a guest name" apart from
    # "person lookup failed" — a real member must never render as a guest.
    person @catch {
      id
      firstName
      lastName
      memberNumber
      location {
        id
        name
      }
    }
  }
`;

export default function ActivityCurrent() {
  const [categoryIds, setCategoryIds] = useState<string[]>([]);
  const [filterText, setFilterText] = useState("");

  return (
    <>
      <p className="my-4">
        This list shows members currently signed in at this location, including
        members visiting from other units (their home unit is shown under their
        name).
      </p>
      <div className="mb-3 flex flex-wrap items-center gap-3">
        <TextInput
          type="text"
          width="half"
          placeholder="Filter by name or member number…"
          value={filterText}
          onChange={(e) => setFilterText(e.target.value)}
        />
        <Suspense fallback={<LoadingIndicator />}>
          <ActivityCategorySelector
            value={categoryIds}
            onChange={setCategoryIds}
          />
        </Suspense>
      </div>
      <Suspense fallback={<LoadingIndicator />}>
        <ActivityCurrentContent
          categoryIds={categoryIds}
          filterText={filterText}
        />
      </Suspense>
    </>
  );
}

function matchesFilter(periodRef: PeriodRef, normalizedFilter: string) {
  if (!normalizedFilter) return true;
  const { person, guestName } = readInlineData<ActivityCurrent_periodName$key>(
    activityCurrentPeriodName,
    periodRef,
  );
  // Non-throwing: a failed person lookup shouldn't hide the row from an empty
  // filter, nor crash filtering for the whole list — it just can't match on
  // name/number, same as a guest can't match on member number.
  if (isValueResult(person) && person.value) {
    const { firstName, lastName, memberNumber } = person.value;
    return (
      `${firstName} ${lastName}`.toLowerCase().includes(normalizedFilter) ||
      (memberNumber?.toLowerCase().includes(normalizedFilter) ?? false)
    );
  }
  return guestName?.toLowerCase().includes(normalizedFilter) ?? false;
}

function ActivityCurrentContent({
  categoryIds,
  filterText,
}: {
  categoryIds: string[];
  filterText: string;
}) {
  const settings = useSettings();
  const relayEnvironment = useRelayEnvironment();
  const data = useRetryableLazyLoadQuery<ActivityCurrentQuery>(
    graphql`
      query ActivityCurrentQuery(
        $location: ID!
        $first: Int!
        $after: String
        $categories: [ID!]
      ) @throwOnFieldError {
        location(id: $location) {
          id
          periods(
            onlyActive: true
            first: $first
            after: $after
            categories: $categories
          ) {
            edges {
              node {
                ...ActivityListTable_period
                ...ActivityCurrent_periodName
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
      first: ACTIVITY_CURRENT_PAGE_SIZE,
      after: null,
      categories: categoryIds.length > 0 ? categoryIds : null,
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
    const { person, guestName } =
      readInlineData<ActivityCurrent_periodName$key>(
        activityCurrentPeriodName,
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

  function getRowSubLabel(periodRef: PeriodRef) {
    const { person } = readInlineData<ActivityCurrent_periodName$key>(
      activityCurrentPeriodName,
      periodRef,
    );
    const personValue = unwrapCatch(person);
    const location = personValue?.location;
    return location && location.id !== settings?.locationId
      ? location.name
      : null;
  }

  async function onLoadMore() {
    if (!hasNextPage || !endCursor || isLoadingMore) {
      return;
    }

    setIsLoadingMore(true);
    setLoadMoreError(null);
    try {
      const next = await fetchQuery<ActivityCurrentQuery>(
        relayEnvironment,
        graphql`
          query ActivityCurrentLoadMoreQuery(
            $location: ID!
            $first: Int!
            $after: String
            $categories: [ID!]
          ) {
            location(id: $location) {
              id
              periods(
                onlyActive: true
                first: $first
                after: $after
                categories: $categories
              ) {
                edges {
                  node {
                    ...ActivityListTable_period
                    ...ActivityCurrent_periodName
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
          first: ACTIVITY_CURRENT_PAGE_SIZE,
          after: endCursor,
          categories: categoryIds.length > 0 ? categoryIds : null,
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

  const normalizedFilter = filterText.trim().toLowerCase();
  const filteredPeriods = normalizedFilter
    ? periods.filter((p) => matchesFilter(p, normalizedFilter))
    : periods;

  return (
    <>
      {normalizedFilter && filteredPeriods.length === 0 && (
        <p className="my-4 text-ink-muted">No periods match “{filterText}”.</p>
      )}
      {normalizedFilter && hasNextPage && (
        <p className="my-4 text-ink-muted">
          Showing matches from the {periods.length} periods loaded so far — use
          Load More below to search further back.
        </p>
      )}
      <ActivityListTable
        firstcol="person"
        periods={filteredPeriods}
        getRowLabel={getRowLabel}
        getRowSubLabel={getRowSubLabel}
        hasNextPage={hasNextPage}
        isLoadingMore={isLoadingMore}
        onLoadMore={onLoadMore}
        loadMoreError={loadMoreError}
      />
    </>
  );
}

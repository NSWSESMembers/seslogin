import { graphql, readInlineData } from "relay-runtime";
import { fetchQuery, useRelayEnvironment } from "react-relay";
import ActivityListTable from "../components/ActivityListTable";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import { unwrapCatch } from "../../lib/relayCatch";
import ActivityCategorySelector from "../components/ActivityCategorySelector";
import LoadingIndicator from "../../components/LoadingIndicator";
import type {
  ActivityListMemberQuery,
  ActivityListMemberQuery$data,
} from "./__generated__/ActivityListMemberQuery.graphql";
import type { ActivityListMember_periodName$key } from "./__generated__/ActivityListMember_periodName.graphql";
import { useParams } from "react-router";
import { Suspense, startTransition, useEffect, useState } from "react";

const ACTIVITY_MEMBER_PAGE_SIZE = 100;

type PeriodRef = NonNullable<
  NonNullable<
    ActivityListMemberQuery$data["person"]["periods"]["edges"][number]
  >["node"]
>;

// This (per-member) view shows the location each period happened at. Colocate
// that data dependency here, read inside getRowLabel from the same period ref.
const activityListMemberPeriodName = graphql`
  fragment ActivityListMember_periodName on Period @inline {
    # @catch so one dangling location reference degrades that row instead of
    # (via @throwOnFieldError on the enclosing query) hiding the whole page.
    # location is non-null in the schema, so a failed lookup here always
    # means an error, never a legitimately absent location.
    location @catch {
      id
      name
    }
  }
`;

export default function ActivityListMember() {
  const params = useParams();
  const [categoryIds, setCategoryIds] = useState<string[]>([]);

  return (
    <>
      <div className="mb-4 flex justify-center">
        <Suspense fallback={<LoadingIndicator />}>
          <ActivityCategorySelector
            value={categoryIds}
            onChange={setCategoryIds}
          />
        </Suspense>
      </div>
      <Suspense fallback={<LoadingIndicator />}>
        <ActivityListMemberContent
          memberId={params.memberId!}
          categoryIds={categoryIds}
        />
      </Suspense>
    </>
  );
}

function ActivityListMemberContent({
  memberId,
  categoryIds,
}: {
  memberId: string;
  categoryIds: string[];
}) {
  const relayEnvironment = useRelayEnvironment();
  const data = useRetryableLazyLoadQuery<ActivityListMemberQuery>(
    graphql`
      query ActivityListMemberQuery(
        $person: ID!
        $first: Int!
        $after: String
        $categories: [ID!]
      ) @throwOnFieldError {
        person(id: $person) {
          id
          firstName
          lastName
          periods(first: $first, after: $after, categories: $categories) {
            edges {
              node {
                ...ActivityListTable_period
                ...ActivityListMember_periodName
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
      person: memberId,
      first: ACTIVITY_MEMBER_PAGE_SIZE,
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
    const nextPeriods = data.person.periods.edges.map((edge) => edge.node);
    startTransition(() => {
      setPeriods(nextPeriods);
      setHasNextPage(data.person.periods.pageInfo.hasNextPage);
      setEndCursor(data.person.periods.pageInfo.endCursor ?? null);
    });
  }, [data.person.id, data.person.periods.edges, data.person.periods.pageInfo]);

  function getRowLabel(periodRef: PeriodRef) {
    const { location } = readInlineData<ActivityListMember_periodName$key>(
      activityListMemberPeriodName,
      periodRef,
    );
    // Throwing unwrap: a failed lookup here is caught by the per-row
    // ErrorBoundary this is always called from within (see ActivityListTable).
    return unwrapCatch(location).name;
  }

  async function onLoadMore() {
    if (!hasNextPage || !endCursor || isLoadingMore) {
      return;
    }

    setIsLoadingMore(true);
    setLoadMoreError(null);
    try {
      const next = await fetchQuery<ActivityListMemberQuery>(
        relayEnvironment,
        graphql`
          query ActivityListMemberLoadMoreQuery(
            $person: ID!
            $first: Int!
            $after: String
            $categories: [ID!]
          ) {
            person(id: $person) {
              id
              firstName
              lastName
              periods(first: $first, after: $after, categories: $categories) {
                edges {
                  node {
                    ...ActivityListTable_period
                    ...ActivityListMember_periodName
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
          person: memberId,
          first: ACTIVITY_MEMBER_PAGE_SIZE,
          after: endCursor,
          categories: categoryIds.length > 0 ? categoryIds : null,
        },
      ).toPromise();

      const nextPeriods =
        next?.person.periods.edges
          // Defensive, same as the initial load: a null edge/node shouldn't be
          // possible per the schema, but a locally mutated store can produce one.
          .filter(
            (edge): edge is NonNullable<typeof edge> => edge?.node != null,
          )
          .map((edge) => edge.node) ?? [];
      setPeriods((previous) => [...previous, ...nextPeriods]);
      setHasNextPage(next?.person.periods.pageInfo.hasNextPage ?? false);
      setEndCursor(next?.person.periods.pageInfo.endCursor ?? null);
    } catch (err) {
      console.error("Failed to load more activity:", err);
      setLoadMoreError("Couldn't load more — please try again.");
    } finally {
      setIsLoadingMore(false);
    }
  }

  return (
    <>
      <p>
        Activity report for: {data.person.firstName} {data.person.lastName}
      </p>
      <ActivityListTable
        firstcol="location"
        periods={periods}
        getRowLabel={getRowLabel}
        hasNextPage={hasNextPage}
        isLoadingMore={isLoadingMore}
        onLoadMore={onLoadMore}
        loadMoreError={loadMoreError}
      />
    </>
  );
}

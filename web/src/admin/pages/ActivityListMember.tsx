import { graphql, readInlineData } from "relay-runtime";
import { fetchQuery, useLazyLoadQuery, useRelayEnvironment } from "react-relay";
import ActivityListTable from "../components/ActivityListTable";
import type {
  ActivityListMemberQuery,
  ActivityListMemberQuery$data,
} from "./__generated__/ActivityListMemberQuery.graphql";
import type { ActivityListMember_periodName$key } from "./__generated__/ActivityListMember_periodName.graphql";
import { useParams } from "react-router";
import { startTransition, useEffect, useState } from "react";

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
    location {
      id
      name
    }
  }
`;

export default function ActivityListMember() {
  const params = useParams();
  const relayEnvironment = useRelayEnvironment();
  const data = useLazyLoadQuery<ActivityListMemberQuery>(
    graphql`
      query ActivityListMemberQuery(
        $person: ID!
        $first: Int!
        $after: String
      ) {
        person(id: $person) {
          id
          firstName
          lastName
          periods(first: $first, after: $after) {
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
      person: params.memberId!,
      first: ACTIVITY_MEMBER_PAGE_SIZE,
      after: null,
    },
  );

  const [periods, setPeriods] = useState<PeriodRef[]>([]);
  const [hasNextPage, setHasNextPage] = useState(false);
  const [endCursor, setEndCursor] = useState<string | null>(null);
  const [isLoadingMore, setIsLoadingMore] = useState(false);

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
    return location?.name ?? "";
  }

  async function onLoadMore() {
    if (!hasNextPage || !endCursor || isLoadingMore) {
      return;
    }

    setIsLoadingMore(true);
    try {
      const next = await fetchQuery<ActivityListMemberQuery>(
        relayEnvironment,
        graphql`
          query ActivityListMemberLoadMoreQuery(
            $person: ID!
            $first: Int!
            $after: String
          ) {
            person(id: $person) {
              id
              firstName
              lastName
              periods(first: $first, after: $after) {
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
          person: params.memberId!,
          first: ACTIVITY_MEMBER_PAGE_SIZE,
          after: endCursor,
        },
      ).toPromise();

      const nextPeriods =
        next?.person.periods.edges.map((edge) => edge.node) ?? [];
      setPeriods((previous) => [...previous, ...nextPeriods]);
      setHasNextPage(next?.person.periods.pageInfo.hasNextPage ?? false);
      setEndCursor(next?.person.periods.pageInfo.endCursor ?? null);
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
      />
    </>
  );
}

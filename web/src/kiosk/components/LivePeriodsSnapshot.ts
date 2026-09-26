import { fetchQuery, graphql, type IEnvironment } from "relay-runtime";
import type { LivePeriodsSnapshotQuery } from "./__generated__/LivePeriodsSnapshotQuery.graphql";
import type { SnapshotEntry } from "../lib/livePeriods";

// Matches the cap every per-consumer query used before this provider existed
// (Status, ScanStatusDialog, ScanSignedInPanel, ScanGuestDialog's GuestList).
const MAX_PERIODS = 100;

const query = graphql`
  query LivePeriodsSnapshotQuery($first: Int!) @throwOnFieldError {
    session {
      location {
        periods(onlyActive: true, first: $first) {
          edges {
            node {
              id
              version
              startTime
              guestName
              person {
                id
                firstName
                lastName
              }
            }
          }
        }
      }
    }
  }
`;

/** Fetches a fresh `periods(onlyActive: true)` snapshot and shapes it into
 * `livePeriods.ts`'s `SnapshotEntry[]`, ready for `applySnapshot`. */
export async function fetchLivePeriodsSnapshot(
  environment: IEnvironment,
): Promise<SnapshotEntry[]> {
  const response = await fetchQuery<LivePeriodsSnapshotQuery>(
    environment,
    query,
    { first: MAX_PERIODS },
    { fetchPolicy: "network-only" },
  ).toPromise();
  const edges = response?.session.location.periods.edges ?? [];
  return edges
    .filter((edge): edge is NonNullable<typeof edge> => edge?.node != null)
    .map(({ node }) => ({
      id: node.id,
      version: node.version,
      name: node.person
        ? `${node.person.firstName} ${node.person.lastName}`
        : (node.guestName ?? "Guest"),
      guest: node.person == null,
      startTime: node.startTime,
    }));
}

import { graphql, useLazyLoadQuery } from "react-relay";
import { useEffect, useState } from "react";
import type { StatusQuery } from "./__generated__/StatusQuery.graphql";
import StatusCurrentDisplay, {
  type StatusPeriod,
} from "../components/StatusCurrentDisplay";

const REFRESH_INTERVAL_MS = 60_000;

export default function Status() {
  const [fetchKey, setFetchKey] = useState(0);

  useEffect(() => {
    const intervalId = window.setInterval(() => {
      setFetchKey((k) => k + 1);
    }, REFRESH_INTERVAL_MS);
    return () => window.clearInterval(intervalId);
  }, []);

  const data = useLazyLoadQuery<StatusQuery>(
    graphql`
      query StatusQuery($first: Int!) {
        session {
          location {
            periods(onlyActive: true, first: $first) {
              edges {
                node {
                  id
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
    `,
    { first: 100 },
    { fetchKey, fetchPolicy: "network-only" },
  );

  // StatusCurrentDisplay is presentational (also rendered with mock data in
  // StatusDemo), so build its plain view model here, reading each field.
  const periods: StatusPeriod[] = data.session.location.periods.edges
    .filter((edge): edge is NonNullable<typeof edge> => edge?.node != null)
    .map(({ node }) => ({
      id: node.id,
      startTime: node.startTime,
      name: node.person
        ? `${node.person.firstName} ${node.person.lastName}`
        : `${node.guestName ?? "Guest"} (Guest)`,
    }));

  return <StatusCurrentDisplay periods={periods} />;
}

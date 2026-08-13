import { useEffect } from "react";
import { fetchQuery, graphql } from "relay-runtime";
import { unauthenticatedEnvironment } from "../lib/environments";
import { setEnvironmentInfo } from "../lib/environmentInfo";
import type { HomeEnvironmentProbeQuery } from "./__generated__/HomeEnvironmentProbeQuery.graphql";

// The selection is handed to setEnvironmentInfo() whole rather than read field by
// field, so the lint rule can't see the usage. See the same disable in
// admin/components/UserInfoProvider.tsx.
/* eslint-disable relay/unused-fields */
const environmentQuery = graphql`
  query HomeEnvironmentProbeQuery {
    environment {
      gitRev
      isProdDb
    }
  }
`;
/* eslint-enable relay/unused-fields */

/**
 * Fetches the API's environment info for the public home page, which has no
 * Relay provider and no credentials.
 *
 * Renders nothing and fires imperatively rather than via `useLazyLoadQuery` so
 * the landing page is never blocked on — or broken by — this request. The result
 * lands in the shared store, which TopBar reads.
 */
export default function HomeEnvironmentProbe() {
  useEffect(() => {
    const subscription = fetchQuery<HomeEnvironmentProbeQuery>(
      unauthenticatedEnvironment,
      environmentQuery,
      {},
    ).subscribe({
      next: (data) => {
        if (data.environment) {
          setEnvironmentInfo(data.environment);
        }
      },
      error: () => undefined,
    });

    return () => subscription.unsubscribe();
  }, []);

  return null;
}

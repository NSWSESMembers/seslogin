import { type ReactNode, useEffect } from "react";
import { useLazyLoadQuery, useRelayEnvironment } from "react-relay";
import { fetchQuery, graphql } from "relay-runtime";
import type { UserInfoProviderQuery } from "./__generated__/UserInfoProviderQuery.graphql";
import type { UserInfoProviderHeartbeatQuery } from "./__generated__/UserInfoProviderHeartbeatQuery.graphql";
import { UserInfoContext } from "./UserInfoContext";
import { setEnvironmentInfo } from "../../lib/environmentInfo";
import { useRelayRetryFetchKey } from "../../components/relayRetryContext";

export { UserInfoContext, type UserInfoContextType } from "./UserInfoContext";

const USER_INFO_RELOAD_INTERVAL_MS = 2 * 60 * 1000;

// These fields are not read in this file: they are exposed app-wide through
// UserInfoContext and consumed via useUserInfo(). That non-Relay channel is
// invisible to the relay/unused-fields lint rule, so disable it here rather than
// fragment-colocating a global current-user across ~10 consumers.
/* eslint-disable relay/unused-fields */
const userInfoQuery = graphql`
  query UserInfoProviderQuery @throwOnFieldError {
    user {
      id
      email
      isSuper
      isDev
      disaggregateVirtualPeriods
      locations {
        id
        name
        enabled
      }
      passkeys {
        __typename
      }
    }
    environment {
      gitRev
      isProdDb
    }
  }
`;

// Deliberately minimal: this only ever needs to make an authenticated round trip so a
// 401 can be noticed (see the interval below), not to carry data. Keeping the passkey
// list and the location grants off it means an idle tab doesn't re-run the
// webauthn_credential index query every two minutes for a value PasskeyEnrollPrompt
// reads exactly once, on mount.
const userInfoHeartbeatQuery = graphql`
  query UserInfoProviderHeartbeatQuery @throwOnFieldError {
    user {
      id
    }
  }
`;
/* eslint-enable relay/unused-fields */

/**
 * Provider component that fetches and provides basic user info via GraphQL
 * Wraps children and makes user data available via useUserInfo() hook
 * Must be wrapped by a Relay Suspense boundary for loading states
 */
export function UserInfoProvider({ children }: { children: ReactNode }) {
  const environment = useRelayEnvironment();
  const fetchKey = useRelayRetryFetchKey();

  useEffect(() => {
    let currentSubscription: { unsubscribe: () => void } | undefined;

    const refreshUserInfo = () => {
      currentSubscription?.unsubscribe();
      currentSubscription = fetchQuery<UserInfoProviderHeartbeatQuery>(
        environment,
        userInfoHeartbeatQuery,
        {},
        {
          // Periodically force a network request so we verify the user's auth token is
          // still valid — a 401 is caught in fetchGraphQL and routed to onUnauthorized()
          // whatever the query was, so the heartbeat needn't fetch anything real.
          //
          // This used to re-run the full userInfoQuery, which also refreshed locations /
          // isSuper / grants in the store as a side effect. It no longer does: those now
          // refresh on page load or on a RelayErrorBoundary retry (both bump the fetch
          // key below).
          fetchPolicy: "network-only",
        },
      ).subscribe({
        next: () => undefined,
        complete: () => undefined,
        error: () => undefined,
      });
    };

    const intervalId = window.setInterval(() => {
      refreshUserInfo();
    }, USER_INFO_RELOAD_INTERVAL_MS);

    return () => {
      window.clearInterval(intervalId);
      currentSubscription?.unsubscribe();
    };
  }, [environment]);

  const data = useLazyLoadQuery<UserInfoProviderQuery>(
    userInfoQuery,
    {},
    {
      fetchPolicy: "store-or-network",
      fetchKey,
    },
  );

  useEffect(() => {
    setEnvironmentInfo(data.environment);
  }, [data.environment]);

  const contextValue = { user: data.user, isLoaded: true };

  return (
    <UserInfoContext.Provider value={contextValue}>
      {children}
    </UserInfoContext.Provider>
  );
}

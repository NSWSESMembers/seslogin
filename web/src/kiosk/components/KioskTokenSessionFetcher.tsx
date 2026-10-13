import { graphql } from "react-relay";
import { fetchQuery } from "relay-runtime";
import type { IEnvironment } from "relay-runtime";
import type { KioskTokenSessionFetcherQuery } from "./__generated__/KioskTokenSessionFetcherQuery.graphql";
import type { KioskSession } from "./KioskSessionContext";
import { setEnvironmentInfo } from "../../lib/environmentInfo";
import {
  recordServerContactFailure,
  recordServerContactSuccess,
} from "../lib/kioskServerStatus";

const SESSION_REFRESH_INTERVAL_MS =
  import.meta.env.MODE === "development"
    ? 20 * 1000 // dev: 20 seconds
    : 2 * 60 * 1000; // prod: 2 minutes

// `environment` is handed to setEnvironmentInfo() whole rather than read field by
// field, so the lint rule can't see the usage. See the same disable in
// admin/components/UserInfoProvider.tsx.
/* eslint-disable relay/unused-fields */
const kioskRefreshQuery = graphql`
  query KioskTokenSessionFetcherQuery {
    refresh_token: refreshToken
    session {
      id
      name
      config
      version
      keyExpiresAt
      location {
        id
        name
      }
    }
    environment {
      gitRev
      isProdDb
    }
  }
`;
/* eslint-enable relay/unused-fields */

export interface KioskSessionFetcher {
  /** Refresh now instead of at the next scheduled poll (then carry on as usual).
   * If a refresh is already in flight, another follows straight after it. */
  refreshNow: () => void;
  stop: () => void;
}

export default function startKioskTokenSessionFetcher({
  environment,
  setToken,
  setSession,
  persistRefreshedToken = true,
}: {
  environment: IEnvironment;
  setToken: (token: string) => void;
  setSession: (session: KioskSession | null) => void;
  // Key-enrolled kiosks authenticate by signing each request, so the JWT that
  // `refreshToken` still returns must not be persisted or used. Set false for them.
  persistRefreshedToken?: boolean;
}): KioskSessionFetcher {
  let timeoutId: number | null = null;
  let isCancelled = false;
  let inFlight = false;
  // Set when `refreshNow` is asked for while a refresh is already in flight: that
  // one may have been answered before whatever prompted the ask, so another
  // follows as soon as it lands.
  let refreshAgain = false;

  const scheduleRefresh = () => {
    if (isCancelled) {
      return;
    }

    const splayAmountMs = SESSION_REFRESH_INTERVAL_MS * 0.1;
    const splayMultiplier = Math.random() * 2 - 1;
    const delayMs = Math.max(
      0,
      SESSION_REFRESH_INTERVAL_MS + splayAmountMs * splayMultiplier,
    );

    timeoutId = window.setTimeout(refresh, delayMs);
  };

  const refresh = () => {
    if (isCancelled || inFlight) {
      return;
    }

    inFlight = true;
    fetchQuery<KioskTokenSessionFetcherQuery>(
      environment,
      kioskRefreshQuery,
      {},
      { fetchPolicy: "network-only" },
    )
      .toPromise()
      .then((response) => {
        inFlight = false;
        recordServerContactSuccess(response?.session?.keyExpiresAt ?? null);
        if (isCancelled) {
          return;
        }

        if (response?.environment) {
          setEnvironmentInfo(response.environment);
        }

        const refreshedToken = response?.refresh_token;
        const refreshedSession = response?.session
          ? {
              id: response.session.id,
              name: response.session.name,
              config: response.session.config ?? {},
              version: response.session.version,
              location: {
                id: response.session.location.id,
                name: response.session.location.name,
              },
            }
          : null;
        if (refreshedToken && persistRefreshedToken) {
          setToken(refreshedToken);
        }
        setSession(refreshedSession);

        scheduleNext();
      })
      .catch((error) => {
        console.log("Failed to refresh kiosk session: ", error);
        recordServerContactFailure(error);
        inFlight = false;
        if (isCancelled) {
          return;
        }

        scheduleNext();
      });
  };

  const scheduleNext = () => {
    if (refreshAgain) {
      refreshAgain = false;
      refresh();
    } else {
      scheduleRefresh();
    }
  };

  refresh();

  return {
    refreshNow: () => {
      if (isCancelled) {
        return;
      }
      if (inFlight) {
        refreshAgain = true;
        return;
      }
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId);
        timeoutId = null;
      }
      refresh();
    },
    stop: () => {
      console.log("Cancelling kiosk session fetcher");
      isCancelled = true;
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId);
      }
    },
  };
}

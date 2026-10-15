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

const DEV = import.meta.env.MODE === "development";

/** How often the kiosk polls its session while nothing pushes changes to it. */
export const SESSION_REFRESH_INTERVAL_MS = DEV
  ? 20 * 1000 // dev: 20 seconds
  : 2 * 60 * 1000; // prod: 2 minutes

/**
 * How often it polls while its session channel is live: admin edits and
 * re-enrollment arrive as pushes then (see SessionPushListener), so the poll is
 * only the heartbeat — it keeps `last_contact` fresh (the admin status dot and
 * dashboard), pings the healthcheck URL, renews the JWT, and proves the kiosk can
 * still authenticate. Their thresholds are sized for this interval.
 */
export const LIVE_SESSION_REFRESH_INTERVAL_MS = DEV
  ? 60 * 1000 // dev: 1 minute
  : 15 * 60 * 1000; // prod: 15 minutes

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
  /** Whether session changes are currently being pushed: the poll stretches to
   * LIVE_SESSION_REFRESH_INTERVAL_MS while they are. Losing the push brings the
   * next poll forward to no later than the ordinary interval from now. */
  setPushLive: (live: boolean) => void;
  stop: () => void;
}

export default function startKioskTokenSessionFetcher({
  environment,
  setToken,
  setSession,
  persistRefreshedToken = true,
  pushLive: initialPushLive = false,
}: {
  environment: IEnvironment;
  setToken: (token: string) => void;
  setSession: (session: KioskSession | null) => void;
  /** Whether session changes are already being pushed when this starts. */
  pushLive?: boolean;
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
  let pushLive = initialPushLive;
  // When the scheduled refresh is due, as `Date.now()` milliseconds.
  let dueAt: number | null = null;

  const scheduleIn = (delayMs: number) => {
    if (timeoutId !== null) {
      window.clearTimeout(timeoutId);
    }
    dueAt = Date.now() + delayMs;
    timeoutId = window.setTimeout(() => {
      timeoutId = null;
      dueAt = null;
      refresh();
    }, delayMs);
  };

  const scheduleRefresh = () => {
    if (isCancelled) {
      return;
    }

    const intervalMs = pushLive
      ? LIVE_SESSION_REFRESH_INTERVAL_MS
      : SESSION_REFRESH_INTERVAL_MS;
    const splayAmountMs = intervalMs * 0.1;
    const splayMultiplier = Math.random() * 2 - 1;
    scheduleIn(Math.max(0, intervalMs + splayAmountMs * splayMultiplier));
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
        dueAt = null;
      }
      refresh();
    },
    setPushLive: (live) => {
      if (isCancelled || live === pushLive) {
        return;
      }
      pushLive = live;
      // A long wait scheduled while pushes were arriving would now leave changes
      // unseen for up to 15 minutes; bring it forward. Going live needs nothing:
      // the next scheduled refresh picks the longer interval itself.
      if (
        !live &&
        dueAt !== null &&
        dueAt - Date.now() > SESSION_REFRESH_INTERVAL_MS
      ) {
        scheduleRefresh();
      }
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

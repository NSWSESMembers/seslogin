import { graphql } from "react-relay";
import { fetchQuery } from "relay-runtime";
import type { IEnvironment } from "relay-runtime";
import type { KioskTokenSessionFetcherQuery } from "./__generated__/KioskTokenSessionFetcherQuery.graphql";
import type { KioskSession } from "./KioskSessionContext";

const SESSION_REFRESH_INTERVAL_MS =
  import.meta.env.MODE === "development"
    ? 20 * 1000 // dev: 20 seconds
    : 2 * 60 * 1000; // prod: 2 minutes

const kioskRefreshQuery = graphql`
  query KioskTokenSessionFetcherQuery {
    refresh_token: refreshToken
    session {
      id
      name
      config
      location {
        id
        name
      }
    }
  }
`;

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
}) {
  let timeoutId: number | null = null;
  let isCancelled = false;
  let inFlight = false;

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
        if (isCancelled) {
          return;
        }

        const refreshedToken = response?.refresh_token;
        const refreshedSession = response?.session
          ? {
              id: response.session.id,
              name: response.session.name,
              config: response.session.config ?? {},
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

        scheduleRefresh();
      })
      .catch((error) => {
        console.log("Failed to refresh kiosk session: ", error);
        inFlight = false;
        if (isCancelled) {
          return;
        }

        scheduleRefresh();
      });
  };

  refresh();

  return () => {
    console.log("Cancelling kiosk session fetcher");
    isCancelled = true;
    if (timeoutId !== null) {
      window.clearTimeout(timeoutId);
    }
  };
}

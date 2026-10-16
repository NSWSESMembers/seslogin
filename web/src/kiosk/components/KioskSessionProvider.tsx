import {
  type ReactNode,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { useRelayEnvironment } from "react-relay";
import LoadingIndicator from "../../components/LoadingIndicator";
import { KioskSessionContext, type KioskSession } from "./KioskSessionContext";
import startKioskSessionFetcher, {
  type KioskSessionFetcher,
} from "./KioskSessionFetcher";
import { KioskRealtimeProvider } from "./KioskRealtimeProvider";
import { LivePeriodsProvider } from "./LivePeriodsProvider";
import { SessionPushListener } from "./SessionPushListener";
import {
  acceptFetchedSession,
  applySessionUpdated,
  type SessionUpdatedPayload,
} from "../lib/sessionPush";

export {
  KioskSessionContext,
  type KioskSessionContextType,
} from "./KioskSessionContext";

function deepEqualSession(
  left: KioskSession | null,
  right: KioskSession | null,
): boolean {
  if (left === right) {
    return true;
  }

  if (left === null || right === null) {
    return false;
  }

  return (
    left.id === right.id &&
    left.name === right.name &&
    left.version === right.version &&
    JSON.stringify(left.config) === JSON.stringify(right.config) &&
    left.location.id === right.location.id &&
    left.location.name === right.location.name
  );
}

/**
 * Provider component that receives and provides kiosk session info.
 * Wraps children and makes session data available via useKioskSession() hook.
 */
export function KioskSessionProvider({ children }: { children: ReactNode }) {
  const environment = useRelayEnvironment();
  const [session, setSession] = useState<KioskSession | null>(null);
  const [isInitialFetchComplete, setIsInitialFetchComplete] = useState(false);

  const fetcher = useRef<KioskSessionFetcher | null>(null);

  const setSessionAndTrackInitialFetch = useCallback(
    (nextSession: KioskSession | null) => {
      setSession((previousSession) =>
        // A fetch that lags a push already applied mustn't roll it back.
        !acceptFetchedSession(previousSession, nextSession) ||
        deepEqualSession(previousSession, nextSession)
          ? previousSession
          : nextSession,
      );
      setIsInitialFetchComplete(true);
    },
    [],
  );

  const applyPushedUpdate = useCallback((push: SessionUpdatedPayload) => {
    setSession((previousSession) => applySessionUpdated(previousSession, push));
  }, []);

  const refreshNow = useCallback(() => fetcher.current?.refreshNow(), []);

  // Kept outside the fetcher too, so a fetcher restarted by the effect below
  // starts on the right interval.
  const pushLive = useRef(false);
  const setPushLive = useCallback((live: boolean) => {
    pushLive.current = live;
    fetcher.current?.setPushLive(live);
  }, []);

  useEffect(() => {
    const started = startKioskSessionFetcher({
      environment,
      setSession: setSessionAndTrackInitialFetch,
      pushLive: pushLive.current,
    });
    fetcher.current = started;
    return () => {
      started.stop();
      fetcher.current = null;
    };
  }, [environment, setSessionAndTrackInitialFetch]);

  // KioskRealtimeProvider and LivePeriodsProvider need both the Relay
  // environment (available above, via KioskKeyRelayEnvironment) and
  // useKioskSession() (only available below, inside KioskSessionContext.Provider)
  // — so they're mounted here rather than in either of those.
  const body = isInitialFetchComplete ? (
    <KioskRealtimeProvider>
      <SessionPushListener
        onUpdated={applyPushedUpdate}
        refreshNow={refreshNow}
        onLiveChange={setPushLive}
      />
      <LivePeriodsProvider>{children}</LivePeriodsProvider>
    </KioskRealtimeProvider>
  ) : (
    <LoadingIndicator />
  );

  return (
    <KioskSessionContext.Provider value={{ session }}>
      {body}
    </KioskSessionContext.Provider>
  );
}

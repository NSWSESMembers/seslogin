import { type ReactNode, useCallback, useEffect, useState } from "react";
import { useRelayEnvironment } from "react-relay";
import LoadingIndicator from "../../components/LoadingIndicator";
import { KioskSessionContext, type KioskSession } from "./KioskSessionContext";
import startKioskTokenSessionFetcher from "./KioskTokenSessionFetcher";
import { LivePeriodsProvider } from "./LivePeriodsProvider";

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
    JSON.stringify(left.config) === JSON.stringify(right.config) &&
    left.location.id === right.location.id &&
    left.location.name === right.location.name
  );
}

/**
 * Provider component that receives and provides kiosk session info.
 * Wraps children and makes session data available via useKioskSession() hook.
 */
export function KioskSessionProvider({
  setToken,
  persistRefreshedToken = true,
  children,
}: {
  setToken: (token: string) => void;
  persistRefreshedToken?: boolean;
  children: ReactNode;
}) {
  const environment = useRelayEnvironment();
  const [session, setSession] = useState<KioskSession | null>(null);
  const [isInitialFetchComplete, setIsInitialFetchComplete] = useState(false);

  const setSessionAndTrackInitialFetch = useCallback(
    (nextSession: KioskSession | null) => {
      setSession((previousSession) =>
        deepEqualSession(previousSession, nextSession)
          ? previousSession
          : nextSession,
      );
      setIsInitialFetchComplete(true);
    },
    [],
  );

  useEffect(() => {
    return startKioskTokenSessionFetcher({
      environment,
      setToken,
      setSession: setSessionAndTrackInitialFetch,
      persistRefreshedToken,
    });
  }, [
    environment,
    setSessionAndTrackInitialFetch,
    setToken,
    persistRefreshedToken,
  ]);

  // LivePeriodsProvider needs both the Relay environment (available above, via
  // KioskRelayEnvironment/KioskKeyRelayEnvironment) and useKioskSession() (only
  // available below, inside KioskSessionContext.Provider) — so it's mounted
  // here rather than in either of those.
  const body = isInitialFetchComplete ? (
    <LivePeriodsProvider>{children}</LivePeriodsProvider>
  ) : (
    <LoadingIndicator />
  );

  return (
    <KioskSessionContext.Provider value={{ session }}>
      {body}
    </KioskSessionContext.Provider>
  );
}

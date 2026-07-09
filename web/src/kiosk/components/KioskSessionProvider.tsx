import { type ReactNode, useCallback, useEffect, useState } from "react";
import { useRelayEnvironment } from "react-relay";
import LoadingIndicator from "../../components/LoadingIndicator";
import { KioskSessionContext, type KioskSession } from "./KioskSessionContext";
import startKioskTokenSessionFetcher from "./KioskTokenSessionFetcher";

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

  const body = isInitialFetchComplete ? children : <LoadingIndicator />;

  return (
    <KioskSessionContext.Provider value={{ session }}>
      {body}
    </KioskSessionContext.Provider>
  );
}

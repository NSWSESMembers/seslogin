import {
  type ReactNode,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { useRelayEnvironment } from "react-relay";
import LoadingIndicator from "../../components/LoadingIndicator";
import {
  Panel,
  PanelBox,
  PanelIntro,
  PanelTitle,
} from "../../components/ui/Panel";
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
  // Set once the server is found unreachable: when the next try is due, or null
  // while it's in flight.
  const [unreachable, setUnreachable] = useState<{
    retryAt: number | null;
  } | null>(null);

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
      onUnreachable: (retryAt) => setUnreachable({ retryAt }),
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
  ) : unreachable ? (
    <KioskUnreachable retryAt={unreachable.retryAt} />
  ) : (
    <LoadingIndicator />
  );

  return (
    <KioskSessionContext.Provider value={{ session }}>
      {body}
    </KioskSessionContext.Provider>
  );
}

/** Before the server has answered once, eg at boot before the network is up. */
function KioskUnreachable({ retryAt }: { retryAt: number | null }) {
  return (
    <Panel>
      <PanelBox>
        <PanelTitle>Can't reach SESLogin</PanelTitle>
        <PanelIntro>
          Check this computer's internet connection. The kiosk carries on by
          itself once it gets through. There's no need to scan a QR code or set
          it up again.
        </PanelIntro>
        <div
          role="status"
          className="flex items-center gap-3 text-lg text-ink-panel"
        >
          <span className="inline-block size-6 animate-spin rounded-full border-[3px] border-line border-t-menu motion-reduce:animate-none" />
          {retryAt === null ? (
            "Trying again…"
          ) : (
            // Keyed so each wait starts counting from its own moment.
            <RetryCountdown key={retryAt} retryAt={retryAt} />
          )}
        </div>
      </PanelBox>
    </Panel>
  );
}

function RetryCountdown({ retryAt }: { retryAt: number }) {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, []);

  const secs = Math.max(1, Math.ceil((retryAt - now) / 1000));
  return <>{`Waiting ${secs} sec${secs === 1 ? "" : "s"}…`}</>;
}

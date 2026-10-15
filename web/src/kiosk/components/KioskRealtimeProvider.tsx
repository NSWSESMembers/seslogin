import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { useRelayEnvironment } from "react-relay";
import { createRealtimeConnection } from "../lib/realtimeClient";
import { hasLiveView } from "../lib/livePeriods";
import { fetchRealtimeToken } from "./KioskRealtimeToken";
import {
  KioskRealtimeContext,
  type KioskRealtimeState,
} from "./KioskRealtimeContext";
import { useKioskSession } from "./useKioskSession";

const OFF: KioskRealtimeState = { status: "off" };
const CONNECTING: KioskRealtimeState = { status: "connecting" };

/**
 * Owns the kiosk's one Ably connection, so every channel the kiosk listens on
 * shares it — Ably bills per connection, not per channel. Consumers
 * (LivePeriodsProvider) take a channel off it with `connection.channel(name)`
 * and release it when done; they never open or close the connection.
 *
 * The token `kioskRealtimeToken` returns names the session's location, so the
 * connection is rebuilt whenever the session or its location changes. It is
 * only opened while something needs it: today, a live "who's signed in" view.
 */
export function KioskRealtimeProvider({ children }: { children: ReactNode }) {
  const environment = useRelayEnvironment();
  const session = useKioskSession();
  const sessionId = session?.id ?? null;
  const locationId = session?.location.id ?? null;
  const enabled = sessionId != null && hasLiveView(session?.config);
  const [generation, setGeneration] = useState(0);

  // Tagged with the connection attempt it describes, so a stale result (from
  // before the session, location or a retry changed things) reads as
  // `connecting` instead of needing a reset inside the effect.
  const attemptKey = `${sessionId}|${locationId}|${generation}`;
  const [result, setResult] = useState<{
    key: string;
    state: KioskRealtimeState;
  } | null>(null);

  useEffect(() => {
    if (!enabled) return;

    let cancelled = false;
    let connection: Awaited<
      ReturnType<typeof createRealtimeConnection>
    > | null = null;

    async function start() {
      let token;
      try {
        token = await fetchRealtimeToken(environment);
      } catch (err) {
        console.error(
          "Failed to fetch kiosk realtime token; falling back to polling",
          err,
        );
        token = null;
      }
      if (cancelled) return;
      if (token == null) {
        setResult({ key: attemptKey, state: { status: "unavailable" } });
        return;
      }

      try {
        const opened = await createRealtimeConnection({
          fetchTokenRequest: async () => {
            const fresh = await fetchRealtimeToken(environment);
            if (fresh == null) {
              throw new Error("Kiosk realtime token is no longer available");
            }
            return fresh.tokenRequest;
          },
        });
        if (cancelled) {
          opened.close();
          return;
        }
        connection = opened;
        setResult({
          key: attemptKey,
          state: {
            status: "ready",
            connection: opened,
            locationChannel: token.channel,
            sessionChannel: token.sessionChannel ?? null,
          },
        });
      } catch (err) {
        if (cancelled) return;
        console.error(
          "Failed to start kiosk realtime connection; falling back to polling",
          err,
        );
        setResult({ key: attemptKey, state: { status: "unavailable" } });
      }
    }

    void start();

    return () => {
      cancelled = true;
      connection?.close();
      // Turning live views off and on again reuses the same attempt key, so
      // forget this attempt's result rather than hand out a closed connection.
      setResult((prev) => (prev?.key === attemptKey ? null : prev));
    };
  }, [environment, enabled, attemptKey]);

  const retry = useCallback(() => setGeneration((g) => g + 1), []);

  // Every branch yields an object that keeps its identity across renders (a
  // constant, or the one stored in `result`), so the memo below only changes
  // when the state does.
  const state: KioskRealtimeState = !enabled
    ? OFF
    : result?.key === attemptKey
      ? result.state
      : CONNECTING;
  const value = useMemo(() => ({ ...state, retry }), [state, retry]);

  return (
    <KioskRealtimeContext.Provider value={value}>
      {children}
    </KioskRealtimeContext.Provider>
  );
}

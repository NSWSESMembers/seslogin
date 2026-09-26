import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { useRelayEnvironment } from "react-relay";
import {
  applyClosed,
  applyOpened,
  applySnapshot,
  createLivePeriodsState,
  EVENT_PERIOD_CLOSED,
  EVENT_PERIOD_OPENED,
  parseClosedPayload,
  parseOpenedPayload,
  selectGuestPeriods,
  selectPeriods,
  type LivePeriodsState,
} from "../lib/livePeriods";
import {
  createRealtimeClient,
  type RealtimeClient,
  type RealtimeMessage,
} from "../lib/realtimeClient";
import { getErrorMessage } from "../../lib/relayErrors";
import { useKioskSession } from "./useKioskSession";
import { LivePeriodsContext, type LiveOwnResult } from "./LivePeriodsContext";
import {
  fetchRealtimeToken,
  type KioskRealtimeTokenPayload,
} from "./KioskRealtimeToken";
import { fetchLivePeriodsSnapshot } from "./LivePeriodsSnapshot";

export {
  LivePeriodsContext,
  type LivePeriodsContextType,
  type LiveOwnResult,
} from "./LivePeriodsContext";

// Fallback cadence while there's no live subscription driving the list —
// matches the cadence every consumer polled at before this provider existed.
const POLL_INTERVAL_MS = 30_000;
// Self-healing resync on an otherwise-healthy subscription, in case a message
// was ever missed without the channel itself reporting discontinuity.
const RESYNC_INTERVAL_MS = 10 * 60 * 1000;

const CHANNEL_FALLBACK_STATES = new Set(["detached", "suspended", "failed"]);
const CONNECTION_FALLBACK_STATES = new Set([
  "disconnected",
  "suspended",
  "failed",
  "closing",
  "closed",
]);

/**
 * Mounted once, under KioskSessionProvider, for every kiosk route — the Relay
 * environment and `useKioskSession()` are both available there. Keeps a live
 * "who's signed in" store fed by the location's Ably channel (falling back to
 * polling when realtime is off or unreachable) and exposes it through
 * `useLivePeriods()` to Status, ScanSignedInPanel, ScanStatusDialog and
 * ScanGuestDialog's GuestList, replacing what used to be four independent
 * polling queries.
 *
 * Design, in one place since no single function tells the whole story:
 * - `kioskRealtimeToken` (see KioskRealtimeToken.ts) returns `null` when
 *   realtime is disabled server-side (no `ABLY_API_KEY`) — that's not an
 *   error, it just means this kiosk polls instead.
 * - Every message and snapshot row carries `Period.version`. `livePeriods.ts`'s
 *   reducer only ever accepts a strictly newer version per period id, and
 *   keeps a closed period as a tombstone rather than deleting it, so an
 *   out-of-order or redelivered message (Ably's guarantee is at-least-once,
 *   not exactly-once-in-order) can't corrupt the list or resurrect something
 *   already closed.
 * - A channel `attached` with `resumed: false` — which includes the very
 *   first attach — means continuity isn't guaranteed, so a fresh snapshot is
 *   fetched. Messages that arrive while that fetch is in flight are buffered
 *   and replayed after, so a period opened after the snapshot request went
 *   out isn't dropped by the snapshot response describing the moment just
 *   before it. A second resync starting before the first finishes (a
 *   non-resumed reattach mid-fetch, or the 10-minute self-heal timer) shares
 *   that same buffer via an in-flight counter, so it's replayed exactly once,
 *   by whichever resync finishes last, instead of an earlier one discarding or
 *   prematurely flushing what the other is still collecting.
 * - Scan/guest mutations apply their own result locally (`applyOwnResult`)
 *   before the Ably echo (or the next poll) would otherwise show it, using the
 *   same version-gated reducer so the echo just coalesces with it. Every entry
 *   `livePeriods.ts` writes is stamped with the local time it was written
 *   (`receivedAt`); a poll or resync snapshot only drops a currently-open entry
 *   that predates when *that* snapshot's fetch was started, so an entry
 *   written locally while the fetch was in flight — an own result, or a live
 *   message applied outside a resync's buffering — survives instead of
 *   flickering out until the next refresh.
 */
export function LivePeriodsProvider({ children }: { children: ReactNode }) {
  const environment = useRelayEnvironment();
  const session = useKioskSession();
  const config = session?.config;
  const active = !!(
    config?.status ||
    config?.signedInStatus ||
    config?.signedInStatusInline ||
    config?.guests
  );
  const locationId = session?.location.id ?? null;

  const [state, setState] = useState<LivePeriodsState>(createLivePeriodsState);
  const [loading, setLoading] = useState(true);
  const [live, setLive] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [retryGeneration, setRetryGeneration] = useState(0);

  const retry = useCallback(() => {
    setError(null);
    setRetryGeneration((g) => g + 1);
  }, []);

  // Applies the kiosk's own scan/guest mutation result immediately, ahead of
  // the Ably echo (or the next poll). Independent of the effect below so it
  // works the instant a mutation completes, regardless of the realtime/poll
  // state machine's phase.
  const applyOwnResult = useCallback((result: LiveOwnResult) => {
    setState((prev) =>
      result.kind === "opened"
        ? applyOpened(prev, result, Date.now())
        : applyClosed(prev, result, Date.now()),
    );
  }, []);

  useEffect(() => {
    // Nothing to synchronize: no consumer needs this data, so no fetch, no
    // subscription, no timers. The public value below already treats
    // `!active` as empty/idle without needing to reset `state` here.
    if (!active) {
      return;
    }

    let cancelled = false;
    let pollTimerId: number | null = null;
    let resyncTimerId: number | null = null;
    let realtimeClient: RealtimeClient | null = null;
    // Non-null while a resync snapshot fetch is in flight: messages arriving
    // during that window are queued here instead of applied immediately, then
    // replayed once the snapshot has been applied — see the doc comment above.
    let messageQueue: RealtimeMessage[] | null = null;
    // Count of resyncs currently in flight. A second resync can start (a
    // non-resumed reattach during the first's fetch, or the 10-minute timer)
    // before the first finishes; the queue above must then survive both, and
    // must only be replayed once, by whichever resync finishes last — see
    // `resyncFromSnapshot`.
    let resyncInFlight = 0;
    // Tracks the channel's own state outside of React, so the 10-minute
    // self-heal timer (below) can tell whether it's actually looking at a
    // healthy subscription. Without this, a resync fired while the channel is
    // down would still succeed (it's a plain HTTP fetch, independent of Ably)
    // and wrongly report `live: true` — the fallback poll would then stop
    // even though the channel itself is still disconnected.
    let channelAttached = false;

    function applyMessage(message: RealtimeMessage) {
      if (cancelled) return;
      if (message.name === EVENT_PERIOD_OPENED) {
        const payload = parseOpenedPayload(message.data);
        if (!payload) {
          console.warn(
            "Ignoring malformed period.opened message",
            message.data,
          );
          return;
        }
        setState((prev) => applyOpened(prev, payload, Date.now()));
      } else if (message.name === EVENT_PERIOD_CLOSED) {
        const payload = parseClosedPayload(message.data);
        if (!payload) {
          console.warn(
            "Ignoring malformed period.closed message",
            message.data,
          );
          return;
        }
        setState((prev) => applyClosed(prev, payload, Date.now()));
      }
    }

    function stopPollingTimer() {
      if (pollTimerId != null) {
        window.clearInterval(pollTimerId);
        pollTimerId = null;
      }
    }

    function startPolling() {
      if (pollTimerId != null) return;
      const poll = () => {
        // Captured before the fetch, not after: an entry written locally (own
        // result, or a live message applied outside a resync) while this
        // request is in flight must survive `applySnapshot` even though it's
        // absent from a response describing an earlier moment — see
        // `applySnapshot`'s doc comment.
        const requestedAtMs = Date.now();
        fetchLivePeriodsSnapshot(environment)
          .then((entries) => {
            if (cancelled) return;
            setState((prev) => applySnapshot(prev, entries, requestedAtMs));
            setLoading(false);
            setError(null);
          })
          .catch((err: unknown) => {
            if (cancelled) return;
            console.error("Kiosk periods poll failed:", err);
            setError(getErrorMessage(err));
            setLoading(false);
          });
      };
      poll();
      pollTimerId = window.setInterval(poll, POLL_INTERVAL_MS);
    }

    async function resyncFromSnapshot() {
      if (cancelled) return;
      // Overlapping resyncs share one queue rather than each owning its own:
      // a second resync (non-resumed reattach mid-fetch, or the 10-minute
      // timer) must not stomp on messages a first, still-in-flight resync is
      // already buffering. `??=` only creates the queue if nothing is
      // buffering yet; an already-in-flight resync's queue is left alone.
      resyncInFlight += 1;
      messageQueue ??= [];
      // See the poll function's comment: captured before the fetch so a
      // locally-written entry from during this window survives `applySnapshot`.
      const requestedAtMs = Date.now();
      try {
        const entries = await fetchLivePeriodsSnapshot(environment);
        if (cancelled) return;
        setState((prev) => applySnapshot(prev, entries, requestedAtMs));
        setError(null);
        setLoading(false);
        // The channel can drop again while this fetch was in flight. When it
        // has, the channel-state handler already reacted (setLive(false),
        // startPolling()) and will resync once more on the next reattach —
        // this snapshot is still applied above, but declaring victory here
        // would wrongly cancel that fallback poll.
        if (channelAttached) {
          stopPollingTimer();
          setLive(true);
        }
      } catch (err) {
        if (cancelled) return;
        console.error(
          "Kiosk realtime resync failed; falling back to polling",
          err,
        );
        setError(getErrorMessage(err));
        setLive(false);
        startPolling();
      } finally {
        resyncInFlight -= 1;
        // Only the last resync to finish flushes the shared queue — an
        // earlier one finishing first must not replay (and null out) messages
        // a still-in-flight sibling is relying on being there afterwards.
        if (resyncInFlight === 0) {
          const queued = messageQueue ?? [];
          messageQueue = null;
          if (!cancelled) {
            queued.forEach(applyMessage);
          }
        }
      }
    }

    async function setupRealtime(token: KioskRealtimeTokenPayload) {
      const client = await createRealtimeClient({
        channel: token.channel,
        fetchTokenRequest: async () => {
          const fresh = await fetchRealtimeToken(environment);
          if (fresh == null) {
            throw new Error("Kiosk realtime token is no longer available");
          }
          return fresh.tokenRequest;
        },
      });
      if (cancelled) {
        client.close();
        return;
      }
      realtimeClient = client;

      client.subscribe((message) => {
        if (messageQueue) {
          messageQueue.push(message);
          return;
        }
        applyMessage(message);
      });

      client.onChannelStateChange((change) => {
        channelAttached = change.current === "attached";
        if (cancelled) return;
        if (change.current === "attached") {
          if (change.resumed) {
            stopPollingTimer();
            setLive(true);
            setLoading(false);
            setError(null);
          } else {
            void resyncFromSnapshot();
          }
        } else if (CHANNEL_FALLBACK_STATES.has(change.current)) {
          setLive(false);
          startPolling();
        }
      });

      client.onConnectionStateChange((change) => {
        if (cancelled) return;
        if (CONNECTION_FALLBACK_STATES.has(change.current)) {
          setLive(false);
          startPolling();
        }
      });

      resyncTimerId = window.setInterval(() => {
        // Only while actually attached — see `channelAttached`'s doc comment.
        // A disconnected channel is already on the polling fallback, which
        // this timer has no business overriding.
        if (channelAttached) {
          void resyncFromSnapshot();
        }
      }, RESYNC_INTERVAL_MS);
    }

    async function start() {
      setLoading(true);
      setError(null);
      setLive(false);

      let token: KioskRealtimeTokenPayload | null;
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
        startPolling();
        return;
      }

      try {
        await setupRealtime(token);
      } catch (err) {
        if (cancelled) return;
        console.error(
          "Failed to start kiosk realtime subscription; falling back to polling",
          err,
        );
        startPolling();
      }
    }

    void start();

    return () => {
      cancelled = true;
      stopPollingTimer();
      if (resyncTimerId != null) {
        window.clearInterval(resyncTimerId);
      }
      realtimeClient?.close();
    };
  }, [environment, active, locationId, retryGeneration]);

  // `active` gates the *public* value rather than the effect resetting
  // internal state: nothing above needs to run for an inactive provider, and
  // this still reports an empty, non-live, non-loading list instead of
  // whatever `state` was left over from the last time it was active.
  const periods = useMemo(
    () => (active ? selectPeriods(state) : []),
    [active, state],
  );
  const guests = useMemo(
    () => (active ? selectGuestPeriods(state) : []),
    [active, state],
  );
  const liveValue = active && live;
  const loadingValue = active && loading;
  const errorValue = active ? error : null;

  const contextValue = useMemo(
    () => ({
      periods,
      guests,
      live: liveValue,
      loading: loadingValue,
      error: errorValue,
      retry,
      applyOwnResult,
    }),
    [
      periods,
      guests,
      liveValue,
      loadingValue,
      errorValue,
      retry,
      applyOwnResult,
    ],
  );

  return (
    <LivePeriodsContext.Provider value={contextValue}>
      {children}
    </LivePeriodsContext.Provider>
  );
}

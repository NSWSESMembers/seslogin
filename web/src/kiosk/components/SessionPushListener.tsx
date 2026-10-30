import { useContext, useEffect, useRef } from "react";
import { fetchKeySessionId } from "../lib/enrollmentKey";
import { getOrCreateKioskKey } from "../lib/kioskKey";
import {
  EVENT_SESSION_CHANGED,
  EVENT_SESSION_UPDATED,
  isNewerPush,
  parseSessionChanged,
  parseSessionUpdated,
  type SessionUpdatedPayload,
} from "../lib/sessionPush";
import { CONFIRM_RETRY_DELAYS_MS } from "../lib/useEnrollmentWatch";
import { KioskEnvironmentContext } from "./KioskEnvironmentContext";
import { useKioskRealtime } from "./useKioskRealtime";
import { useKioskSession } from "./useKioskSession";

/**
 * Listens on this kiosk's own session channel (from KioskRealtimeProvider)
 * and keeps the session current without waiting for the next poll:
 *
 * - `session.updated` (an admin edit) is applied straight away, through
 *   `onUpdated`, which drops anything not newer than what's shown.
 * - `session.changed` (deleted, or the key released to a newly enrolled
 *   session) and any non-resumed attach — continuity lost, so something may
 *   have been missed — call `refreshNow` for an authenticated refetch.
 *
 * A key-mode kiosk confirms before it refetches after `session.changed`. The
 * server resolves its key to a session through an eventually consistent index,
 * so straight after a re-enrollment that index can still find no session, and
 * a 401 from the refetch would drop the kiosk onto the enrollment screen. So
 * it first asks, with a side request that can't trigger that, until the key
 * resolves to a different session — the same backoff `useEnrollmentWatch`
 * uses. If it never does (the session was deleted), the refetch goes ahead
 * anyway and its 401 takes the kiosk to enrollment, which is then right.
 *
 * It also reports, through `onLiveChange`, whether pushes are flowing right now
 * (channel attached and connection up), which is what lets the session poll
 * stretch out.
 */
export function SessionPushListener({
  onUpdated,
  refreshNow,
  onLiveChange,
}: {
  onUpdated: (push: SessionUpdatedPayload) => void;
  refreshNow: () => void;
  onLiveChange: (live: boolean) => void;
}) {
  const session = useKioskSession();
  const realtime = useKioskRealtime();
  const kioskEnvironment = useContext(KioskEnvironmentContext);
  const connection = realtime.status === "ready" ? realtime.connection : null;
  const channelName =
    realtime.status === "ready" ? realtime.sessionChannel : null;

  // The subscription outlives renders; read the latest of these through refs.
  const latest = useRef({
    session,
    onUpdated,
    refreshNow,
    onLiveChange,
    kioskEnvironment,
  });
  useEffect(() => {
    latest.current = {
      session,
      onUpdated,
      refreshNow,
      onLiveChange,
      kioskEnvironment,
    };
  });

  useEffect(() => {
    if (connection == null || channelName == null) return;

    let cancelled = false;
    const channel = connection.channel(channelName);
    // Pushes are only arriving while both hold. A disconnected connection can
    // leave the channel reading "attached" for a while before it's suspended,
    // so the channel's state alone isn't enough.
    let channelAttached = false;
    let connected = connection.state === "connected";
    const reportLive = () =>
      latest.current.onLiveChange(channelAttached && connected);

    async function handleChanged(sessionId: string) {
      const env = latest.current.kioskEnvironment;
      if (env) {
        await waitForKeyToMove(env.profile, sessionId, () => cancelled);
      }
      if (!cancelled) latest.current.refreshNow();
    }

    channel.subscribe((message) => {
      if (cancelled) return;
      if (message.name === EVENT_SESSION_UPDATED) {
        const push = parseSessionUpdated(message.data);
        if (!push) {
          console.warn("Ignoring malformed session.updated", message.data);
          return;
        }
        latest.current.onUpdated(push);
      } else if (message.name === EVENT_SESSION_CHANGED) {
        const push = parseSessionChanged(message.data);
        if (!push) {
          console.warn("Ignoring malformed session.changed", message.data);
          return;
        }
        if (isNewerPush(latest.current.session, push.sessionId, push.version)) {
          void handleChanged(push.sessionId);
        }
      }
    });

    channel.onChannelStateChange((change) => {
      if (cancelled) return;
      channelAttached = change.current === "attached";
      reportLive();
      // Includes the first attach: a change made between the session's last
      // fetch and this subscription going live would otherwise wait for the
      // next poll.
      if (change.current === "attached" && !change.resumed) {
        latest.current.refreshNow();
      }
    });

    const stopConnectionListener = connection.onConnectionStateChange(
      (change) => {
        if (cancelled) return;
        connected = change.current === "connected";
        reportLive();
      },
    );

    return () => {
      cancelled = true;
      stopConnectionListener();
      channel.release();
      latest.current.onLiveChange(false);
    };
  }, [connection, channelName]);

  return null;
}

/** Resolves once this device's key authenticates as a session other than
 * `fromSessionId`, or after the last retry if it never does. */
async function waitForKeyToMove(
  profile: string,
  fromSessionId: string,
  isCancelled: () => boolean,
): Promise<void> {
  let info;
  try {
    info = await getOrCreateKioskKey(profile);
  } catch (err) {
    console.error("Failed to load kiosk key to confirm a session change", err);
    return;
  }
  for (const delay of [0, ...CONFIRM_RETRY_DELAYS_MS]) {
    if (delay > 0) {
      await new Promise((resolve) => window.setTimeout(resolve, delay));
    }
    if (isCancelled()) return;
    let sessionId: string | null = null;
    try {
      sessionId = await fetchKeySessionId(info);
    } catch (err) {
      console.error("Session change confirm failed:", err);
    }
    if (sessionId != null && sessionId !== fromSessionId) return;
  }
}

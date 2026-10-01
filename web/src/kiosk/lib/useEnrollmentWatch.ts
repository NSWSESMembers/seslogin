import { useEffect, useRef } from "react";
import type { KioskKeyInfo } from "./kioskKey";
import {
  EVENT_ENROLLMENT_COMPLETED,
  fetchEnrollmentRealtimeToken,
  fetchKeySessionId,
} from "./enrollmentKey";
import { pollDelayMs } from "./enrollPolling";
import { createRealtimeClient, type RealtimeClient } from "./realtimeClient";

/**
 * Watches for an admin enrolling (or reactivating) this device's key, and calls
 * `onEnrolled` once it has been.
 *
 * Design:
 * - Push first. Once the key has been published, the server hands out a
 *   subscribe-only Ably token for a per-key channel and publishes
 *   `enrollment.completed` there when enrollment finishes, so in the normal case
 *   there is no polling at all.
 * - Fallback poll. If realtime is unavailable — the server has no Ably key (token
 *   is null), the token request fails, the client can't be created, or the channel
 *   or connection later drops — we poll `fetchKeySessionId` on the `pollDelayMs`
 *   backoff, the same cadence the screen used before realtime existed. The poll
 *   stops again once the channel is attached.
 * - Confirm, don't trust. The message is content-free; it only means "check now".
 *   We confirm with `fetchKeySessionId`, a request signed with this device's own
 *   key, so nothing on the channel can make a kiosk believe it is enrolled.
 * - Retry the confirm. The server resolves a key to its session through a
 *   DynamoDB GSI, which is eventually consistent, so the first check right after
 *   the push can still be refused. We retry a few times on a short backoff before
 *   giving up and waiting for the next signal.
 * - Close the race. A channel attach that is not a resume (including the first
 *   attach) triggers one check, covering enrollment that happened between the
 *   initial check and the subscription going live.
 *
 * `onEnrolled` is called at most once. Set `enabled` only once the key has been
 * published (see `useEnrollmentQr().published`).
 */

/** Delays between confirm attempts after an enrollment push. */
export const CONFIRM_RETRY_DELAYS_MS = [500, 1000, 2000, 4000, 8000];

const CHANNEL_FALLBACK_STATES = new Set(["detached", "suspended", "failed"]);
const CONNECTION_FALLBACK_STATES = new Set(["failed", "suspended", "closed"]);

export function useEnrollmentWatch({
  info,
  enabled,
  onEnrolled,
}: {
  info: KioskKeyInfo | null;
  enabled: boolean;
  onEnrolled: () => void;
}): void {
  // Callers pass inline callbacks; keep the effect from restarting on each render.
  const onEnrolledRef = useRef(onEnrolled);
  useEffect(() => {
    onEnrolledRef.current = onEnrolled;
  });

  useEffect(() => {
    if (!enabled || info == null) return;

    let cancelled = false;
    let done = false;
    let closing = false;
    let client: RealtimeClient | null = null;
    let pollTimeout: number | null = null;
    let polling = false;
    let confirmTimeout: number | null = null;
    let confirmRunning = false;
    let confirmRetry = false;
    let confirmAttempt = 0;
    const startedAt = Date.now();

    function finish() {
      if (cancelled || done) return;
      done = true;
      stopPolling();
      clearConfirmTimer();
      onEnrolledRef.current();
    }

    async function check(logLabel: string): Promise<boolean> {
      try {
        return (await fetchKeySessionId(info!)) != null;
      } catch (err) {
        console.error(`${logLabel} failed:`, err);
        return false;
      }
    }

    function stopPolling() {
      polling = false;
      if (pollTimeout !== null) {
        window.clearTimeout(pollTimeout);
        pollTimeout = null;
      }
    }

    function startPolling() {
      if (polling || cancelled || done) return;
      polling = true;
      const runPoll = async () => {
        pollTimeout = null;
        if (cancelled || done || !polling) return;
        const enrolled = await check("Enrollment poll");
        if (cancelled || done || !polling) return;
        if (enrolled) {
          finish();
          return;
        }
        pollTimeout = window.setTimeout(
          runPoll,
          pollDelayMs(Date.now() - startedAt),
        );
      };
      void runPoll();
    }

    function clearConfirmTimer() {
      if (confirmTimeout !== null) {
        window.clearTimeout(confirmTimeout);
        confirmTimeout = null;
      }
    }

    // Confirm after a signal. After an enrollment push (`retry`) the check is retried
    // on a short backoff (see header comment); after a plain attach it is a single
    // check, since nothing says enrollment has happened. A push that lands while a
    // single check is in flight upgrades it to the retrying kind rather than being
    // dropped, since that check may already have raced the GSI and lost.
    function confirm(retry: boolean) {
      if (cancelled || done) return;
      if (retry && !confirmRetry) {
        confirmRetry = true;
        confirmAttempt = 0;
      }
      if (confirmRunning) return;
      confirmRunning = true;
      const run = async () => {
        confirmTimeout = null;
        if (cancelled || done) return;
        const enrolled = await check("Enrollment confirm");
        if (cancelled || done) return;
        if (enrolled) {
          confirmRunning = false;
          finish();
          return;
        }
        if (confirmRetry && confirmAttempt < CONFIRM_RETRY_DELAYS_MS.length) {
          confirmTimeout = window.setTimeout(
            run,
            CONFIRM_RETRY_DELAYS_MS[confirmAttempt++],
          );
        } else {
          // Give up until the next signal.
          confirmRunning = false;
          confirmRetry = false;
        }
      };
      void run();
    }

    async function start() {
      // Covers a key that is already enrolled.
      if (await check("Initial enrollment check")) {
        finish();
        return;
      }
      if (cancelled || done) return;

      let token;
      try {
        token = await fetchEnrollmentRealtimeToken(info!);
      } catch (err) {
        console.error(
          "Failed to fetch enrollment realtime token; falling back to polling",
          err,
        );
        token = null;
      }
      if (cancelled || done) return;
      if (token == null) {
        startPolling();
        return;
      }

      let created: RealtimeClient;
      try {
        created = await createRealtimeClient({
          channel: token.channel,
          fetchTokenRequest: async () => {
            const fresh = await fetchEnrollmentRealtimeToken(info!);
            if (fresh == null) {
              throw new Error(
                "Enrollment realtime token is no longer available",
              );
            }
            return fresh.tokenRequest;
          },
        });
      } catch (err) {
        if (cancelled) return;
        console.error(
          "Failed to start enrollment realtime subscription; falling back to polling",
          err,
        );
        startPolling();
        return;
      }
      if (cancelled || done) {
        closing = true;
        created.close();
        return;
      }
      client = created;

      client.subscribe((message) => {
        if (message.name === EVENT_ENROLLMENT_COMPLETED) {
          confirm(true);
        }
      });

      client.onChannelStateChange((change) => {
        if (cancelled || done) return;
        if (change.current === "attached") {
          stopPolling();
          if (!change.resumed) confirm(false);
        } else if (CHANNEL_FALLBACK_STATES.has(change.current)) {
          startPolling();
        }
      });

      client.onConnectionStateChange((change) => {
        if (cancelled || done || closing) return;
        if (CONNECTION_FALLBACK_STATES.has(change.current)) startPolling();
      });
    }

    void start();

    return () => {
      cancelled = true;
      closing = true;
      stopPolling();
      clearConfirmTimer();
      client?.close();
    };
  }, [info, enabled]);
}

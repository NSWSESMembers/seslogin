// Thin seam over ably-js. LivePeriodsProvider talks only to this file's types,
// never to "ably" directly, so a test can inject a fake implementation (via
// `vi.mock("./realtimeClient")`) with no real network and no jsdom WebSocket.
// The real "ably" package is imported lazily — see createRealtimeClient — so
// importing this module (or LivePeriodsProvider) doesn't pull it into a bundle
// that never turns realtime on.

/** Mirrors the subset of Ably's `AblyTokenRequest` GraphQL type
 * (`kioskRealtimeToken`) that ably-js's `authCallback` needs. */
export interface RealtimeTokenRequest {
  keyName: string;
  ttl: number;
  capability: string;
  clientId: string;
  timestamp: number;
  nonce: string;
  mac: string;
}

export interface RealtimeMessage {
  name: string;
  data: unknown;
}

export interface RealtimeChannelStateChange {
  /** e.g. "attaching" | "attached" | "detached" | "suspended" | "failed". */
  current: string;
  /** True when the channel resumed message continuity across a brief
   * disconnect. False (including on the very first attach) means a snapshot
   * is needed before trusting any messages that follow. */
  resumed: boolean;
}

export interface RealtimeConnectionStateChange {
  /** e.g. "connecting" | "connected" | "disconnected" | "suspended" | "failed" | "closed". */
  current: string;
}

export interface RealtimeClient {
  /** Registers a listener for every message on the channel (both
   * `period.opened` and `period.closed` — the caller dispatches on `name`). */
  subscribe(listener: (message: RealtimeMessage) => void): void;
  onChannelStateChange(
    listener: (change: RealtimeChannelStateChange) => void,
  ): void;
  onConnectionStateChange(
    listener: (change: RealtimeConnectionStateChange) => void,
  ): void;
  /** Closes the underlying connection. Idempotent-ish: safe to call once on
   * cleanup; not meant to be called twice. */
  close(): void;
}

export interface CreateRealtimeClientOptions {
  channel: string;
  /**
   * Called by ably-js whenever it needs a (fresh) token — on connect and again
   * before the current one expires. Must resolve with a `RealtimeTokenRequest`
   * or reject; this seam turns a rejection into ably-js's `callback(error,
   * null)` convention itself, so `createRealtimeClient` never needs to catch a
   * throw from inside ably-js's own auth machinery.
   */
  fetchTokenRequest: () => Promise<RealtimeTokenRequest>;
}

/** Creates a realtime client connected with token auth and subscribed to
 * `opts.channel`. Ably is imported lazily so a kiosk that never enables a
 * status/guest view (or a non-kiosk page entirely) never loads it. */
export async function createRealtimeClient(
  opts: CreateRealtimeClientOptions,
): Promise<RealtimeClient> {
  const Ably = await import("ably");

  const realtime = new Ably.Realtime({
    authCallback: (_tokenParams, callback) => {
      opts
        .fetchTokenRequest()
        .then((tokenRequest) => callback(null, tokenRequest))
        .catch((err: unknown) => {
          // ably-js's contract: report failure through the callback, never throw
          // — a throw here would be an unhandled error inside ably-js's own retry
          // loop rather than a renewal it can react to.
          callback(err instanceof Error ? err.message : String(err), null);
        });
    },
  });
  const channel = realtime.channels.get(opts.channel);

  return {
    subscribe(listener) {
      // Implicitly attaches the channel. Fire-and-forget: the returned promise
      // only reflects the attach, which onChannelStateChange already reports
      // — the empty catch just stops a failed attach from surfacing as an
      // unhandled promise rejection.
      channel
        .subscribe((message) => {
          listener({ name: message.name ?? "", data: message.data });
        })
        .catch(() => {});
    },
    onChannelStateChange(listener) {
      channel.on((change) => {
        listener({ current: change.current, resumed: change.resumed });
      });
    },
    onConnectionStateChange(listener) {
      realtime.connection.on((change) => {
        listener({ current: change.current });
      });
    },
    close() {
      realtime.close();
    },
  };
}

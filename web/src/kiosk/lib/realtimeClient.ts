// Thin seam over ably-js. KioskRealtimeProvider, LivePeriodsProvider and
// useEnrollmentWatch talk only to this file's types, never to "ably" directly,
// so a test can inject a fake implementation (via `vi.mock("./realtimeClient")`)
// with no real network and no jsdom WebSocket. The real "ably" package is
// imported lazily — see createRealtimeConnection — so importing this module
// doesn't pull it into a bundle that never turns realtime on.

// Type-only, so erased at build time: it doesn't defeat the lazy import below.
import type {
  ChannelStateChange,
  ConnectionStateChange,
  InboundMessage,
} from "ably";

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

/** One channel on a shared {@link RealtimeConnection}. */
export interface RealtimeChannel {
  /** Registers a listener for every message on the channel; the caller
   * dispatches on `name`. Implicitly attaches the channel. */
  subscribe(listener: (message: RealtimeMessage) => void): void;
  onChannelStateChange(
    listener: (change: RealtimeChannelStateChange) => void,
  ): void;
  /** Drops this handle's listeners and detaches the channel, leaving the
   * connection open for its other channels. Hold at most one handle per
   * channel name at a time: releasing detaches the channel for everyone. */
  release(): void;
}

/** One Ably connection, shared by every channel a kiosk subscribes to — Ably
 * bills per connection, not per channel. */
export interface RealtimeConnection {
  /** The connection's current state, for a listener registered after it was
   * reached (state changes are only reported as they happen). */
  readonly state: string;
  channel(name: string): RealtimeChannel;
  /** Returns a function that removes the listener again. */
  onConnectionStateChange(
    listener: (change: RealtimeConnectionStateChange) => void,
  ): () => void;
  close(): void;
}

export interface CreateRealtimeConnectionOptions {
  /**
   * Called by ably-js whenever it needs a (fresh) token — on connect and again
   * before the current one expires. Must resolve with a `RealtimeTokenRequest`
   * or reject; this seam turns a rejection into ably-js's `callback(error,
   * null)` convention itself, so `createRealtimeClient` never needs to catch a
   * throw from inside ably-js's own auth machinery.
   */
  fetchTokenRequest: () => Promise<RealtimeTokenRequest>;
}

export interface CreateRealtimeClientOptions extends CreateRealtimeConnectionOptions {
  channel: string;
}

/** Opens a realtime connection with token auth. Ably is imported lazily so a
 * page that never turns realtime on never loads it. */
export async function createRealtimeConnection(
  opts: CreateRealtimeConnectionOptions,
): Promise<RealtimeConnection> {
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

  return {
    get state() {
      return realtime.connection.state;
    },
    channel(name) {
      const channel = realtime.channels.get(name);
      const messageListeners: Array<(message: InboundMessage) => void> = [];
      const stateListeners: Array<(change: ChannelStateChange) => void> = [];
      return {
        subscribe(listener) {
          const wrapped = (message: InboundMessage) => {
            listener({ name: message.name ?? "", data: message.data });
          };
          messageListeners.push(wrapped);
          // Implicitly attaches the channel. Fire-and-forget: the returned
          // promise only reflects the attach, which onChannelStateChange
          // already reports — the empty catch just stops a failed attach from
          // surfacing as an unhandled promise rejection.
          channel.subscribe(wrapped).catch(() => {});
        },
        onChannelStateChange(listener) {
          const wrapped = (change: ChannelStateChange) => {
            listener({ current: change.current, resumed: change.resumed });
          };
          stateListeners.push(wrapped);
          channel.on(wrapped);
        },
        release() {
          messageListeners.forEach((l) => channel.unsubscribe(l));
          stateListeners.forEach((l) => channel.off(l));
          // Same reasoning as subscribe's empty catch.
          channel.detach().catch(() => {});
        },
      };
    },
    onConnectionStateChange(listener) {
      const wrapped = (change: ConnectionStateChange) => {
        listener({ current: change.current });
      };
      realtime.connection.on(wrapped);
      return () => realtime.connection.off(wrapped);
    },
    close() {
      realtime.close();
    },
  };
}

/** A connection with a single channel on it, for a caller that owns both —
 * the enrollment screen, which has no session and so nothing to share. */
export async function createRealtimeClient(
  opts: CreateRealtimeClientOptions,
): Promise<RealtimeClient> {
  const connection = await createRealtimeConnection(opts);
  const channel = connection.channel(opts.channel);
  return {
    subscribe: (listener) => channel.subscribe(listener),
    onChannelStateChange: (listener) => channel.onChannelStateChange(listener),
    onConnectionStateChange: (listener) => {
      connection.onConnectionStateChange(listener);
    },
    close: () => connection.close(),
  };
}

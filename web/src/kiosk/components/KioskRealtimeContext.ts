import { createContext } from "react";
import type { RealtimeConnection } from "../lib/realtimeClient";

/** The kiosk's one shared Ably connection, as KioskRealtimeProvider sees it. */
export type KioskRealtimeState =
  /** Nothing on this kiosk needs realtime, so no connection is open. */
  | { status: "off" }
  /** Fetching a token and opening the connection. */
  | { status: "connecting" }
  /** Realtime is disabled server-side (no `ABLY_API_KEY`), or the token or
   * connection couldn't be had: consumers poll. Not an error state — `retry`
   * tries again. */
  | { status: "unavailable" }
  | {
      status: "ready";
      connection: RealtimeConnection;
      /** This kiosk's location channel (period opened/closed events). */
      locationChannel: string;
      /** This kiosk's own session channel (`session.updated` /
       * `session.changed`). Null against a server that predates it. */
      sessionChannel: string | null;
    };

export type KioskRealtimeContextType = KioskRealtimeState & {
  /** Fetch a fresh token and reconnect. Only meaningful while `unavailable`. */
  retry: () => void;
};

export const KioskRealtimeContext = createContext<
  KioskRealtimeContextType | undefined
>(undefined);

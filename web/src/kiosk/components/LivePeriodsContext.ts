import { createContext } from "react";
import type { StatusPeriod } from "./StatusCurrentDisplay";
import type {
  PeriodClosedPayload,
  PeriodOpenedPayload,
} from "../lib/livePeriods";

/** What a kiosk applies locally right after one of its own scan mutations
 * completes — see LivePeriodsProvider's `applyOwnResult`. Same wire shape Ably
 * uses, so the eventual echo just coalesces by version instead of double-adding. */
export type LiveOwnResult =
  | ({ kind: "opened" } & PeriodOpenedPayload)
  | ({ kind: "closed" } & PeriodClosedPayload);

export interface LivePeriodsContextType {
  /** Everyone currently signed in at this location, oldest sign-in first. */
  periods: StatusPeriod[];
  /** Currently signed-in guests only, oldest first. */
  guests: StatusPeriod[];
  /** True while the list is being kept live by the Ably subscription; false
   * while on the polling fallback (realtime disabled server-side, or the
   * connection/channel isn't currently attached). Purely informational — every
   * consumer reads the same `periods`/`guests` either way. */
  live: boolean;
  /** True until the first snapshot (a poll, or a realtime resync) completes. */
  loading: boolean;
  /** Set when the most recent snapshot attempt failed; cleared on the next
   * success. Doesn't clear `periods`/`guests` — the list just goes stale. */
  error: string | null;
  /** Clears `error` and restarts the token fetch / subscribe / poll cycle. */
  retry: () => void;
  /** Merges the kiosk's own scan/guest mutation result into the live list right
   * away, so it shows before the Ably echo (or the next poll) would otherwise
   * bring it in. */
  applyOwnResult: (result: LiveOwnResult) => void;
}

export const LivePeriodsContext = createContext<
  LivePeriodsContextType | undefined
>(undefined);

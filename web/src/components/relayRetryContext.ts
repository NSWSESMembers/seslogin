import { createContext, useContext } from "react";

/**
 * Bumped every time "Try again" is clicked on the nearest RelayErrorBoundary;
 * see useRelayRetryFetchKey below for why a component needs this to actually
 * retry.
 */
export const RelayRetryContext = createContext(0);

/**
 * A query component whose "Try again" retry must genuinely hit the network
 * should pass `fetchKey: useRelayRetryFetchKey()` to its `useLazyLoadQuery`
 * call. See RelayErrorBoundary's doc comment for why invalidating the store
 * alone isn't enough.
 *
 * Returns 0 (a no-op fetchKey) outside any RelayErrorBoundary, so it's safe
 * to call unconditionally.
 */
export function useRelayRetryFetchKey(): number {
  return useContext(RelayRetryContext);
}

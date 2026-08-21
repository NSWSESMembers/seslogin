import { useState, type ReactNode } from "react";
import { ErrorBoundary, type FallbackProps } from "react-error-boundary";
import { commitLocalUpdate } from "relay-runtime";
import { useRelayEnvironment } from "react-relay";
import PageErrorFallback from "./PageErrorFallback";
import { RelayRetryContext } from "./relayRetryContext";
import { useCaughtErrorMessage } from "../lib/useCaughtErrorMessage";

interface RelayErrorBoundaryProps {
  children: ReactNode;
  /** Remounts the boundary (clearing its caught error) when this value changes. */
  resetKey?: string | number;
  showDetailsByDefault?: boolean;
  /**
   * Set only once every `useLazyLoadQuery` call reachable from `children`
   * threads `useRelayRetryFetchKey()` into its `fetchKey` option — that's
   * what makes "Try again" actually retry (see the doc comment below).
   * Without it, "Try again" looks like it does something but doesn't:
   * default to a "Reload page" button that does a real `window.location.reload()`
   * instead.
   */
  canRetry?: boolean;
}

/**
 * An ErrorBoundary for use anywhere inside a RelayEnvironmentProvider.
 *
 * A bare `resetErrorBoundary` only resets React state, and invalidating the
 * store on reset isn't enough either: `useLazyLoadQuery`'s underlying
 * QueryResource caches the outcome of a query — success, pending promise, or
 * thrown error — keyed by (fetchPolicy, renderPolicy, operation identifier),
 * which is independent of the store's invalidation epoch entirely. On retry
 * with the same variables, that cache entry is still there holding the
 * original error, so it's rethrown synchronously with no network request,
 * regardless of what the store thinks is stale. Store invalidation still
 * matters for making a *fresh* cache entry correctly treat existing store
 * data as stale rather than reusing it, so both run on reset:
 * - `store.invalidateStore()` bumps the store's invalidation epoch.
 * - `retryGeneration` increments, so a query component that threads it into
 *   `fetchKey` (via useRelayRetryFetchKey) gets a fresh QueryResource cache
 *   entry — Relay's documented mechanism for forcing a query to be
 *   "re-evaluated... even if the variables didn't change" — which is what
 *   actually triggers a new fetch.
 *
 * Neither of those helps a single bit if nothing under `children` reads
 * `useRelayRetryFetchKey()`, which is the case for most of this app today.
 * `canRetry` defaults to false so a boundary that hasn't been checked for
 * that shows an honest "Reload page" instead of a "Try again" that silently
 * rethrows the same cached error.
 */
export default function RelayErrorBoundary({
  children,
  resetKey,
  showDetailsByDefault,
  canRetry = false,
}: RelayErrorBoundaryProps) {
  const environment = useRelayEnvironment();
  const [retryGeneration, setRetryGeneration] = useState(0);
  const { message, onError, reset } = useCaughtErrorMessage();

  return (
    <ErrorBoundary
      key={resetKey}
      onError={onError}
      onReset={() => {
        reset();
        commitLocalUpdate(environment, (store) => store.invalidateStore());
        setRetryGeneration((n) => n + 1);
      }}
      fallbackRender={({ error, resetErrorBoundary }: FallbackProps) => (
        <PageErrorFallback
          error={error}
          resetErrorBoundary={resetErrorBoundary}
          showDetailsByDefault={showDetailsByDefault}
          reloadInstead={!canRetry}
          message={message ?? undefined}
        />
      )}
    >
      <RelayRetryContext.Provider value={retryGeneration}>
        {children}
      </RelayRetryContext.Provider>
    </ErrorBoundary>
  );
}

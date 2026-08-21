import { useState, type ReactNode } from "react";
import { ErrorBoundary, type FallbackProps } from "react-error-boundary";
import { commitLocalUpdate } from "relay-runtime";
import { useRelayEnvironment } from "react-relay";
import PageErrorFallback from "./PageErrorFallback";
import { RelayRetryContext } from "./relayRetryContext";

interface RelayErrorBoundaryProps {
  children: ReactNode;
  /** Remounts the boundary (clearing its caught error) when this value changes. */
  resetKey?: string | number;
  showDetailsByDefault?: boolean;
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
 */
export default function RelayErrorBoundary({
  children,
  resetKey,
  showDetailsByDefault,
}: RelayErrorBoundaryProps) {
  const environment = useRelayEnvironment();
  const [retryGeneration, setRetryGeneration] = useState(0);

  return (
    <ErrorBoundary
      key={resetKey}
      onReset={() => {
        commitLocalUpdate(environment, (store) => store.invalidateStore());
        setRetryGeneration((n) => n + 1);
      }}
      fallbackRender={({ error, resetErrorBoundary }: FallbackProps) => (
        <PageErrorFallback
          error={error}
          resetErrorBoundary={resetErrorBoundary}
          showDetailsByDefault={showDetailsByDefault}
        />
      )}
    >
      <RelayRetryContext.Provider value={retryGeneration}>
        {children}
      </RelayRetryContext.Provider>
    </ErrorBoundary>
  );
}

import type { ReactNode } from "react";
import { ErrorBoundary, type FallbackProps } from "react-error-boundary";
import { commitLocalUpdate } from "relay-runtime";
import { useRelayEnvironment } from "react-relay";
import PageErrorFallback from "./PageErrorFallback";

interface RelayErrorBoundaryProps {
  children: ReactNode;
  /** Remounts the boundary (clearing its caught error) when this value changes. */
  resetKey?: string | number;
  showDetailsByDefault?: boolean;
}

/**
 * An ErrorBoundary whose "Try again" button actually refetches, for use anywhere
 * inside a RelayEnvironmentProvider.
 *
 * A bare `resetErrorBoundary` only resets React state — `useLazyLoadQuery`'s default
 * fetchPolicy (`store-or-network`) then re-reads the same record from the store,
 * which still holds whatever partial or errored data caused the throw, and throws
 * again. Invalidating the whole store on reset bumps its global invalidation epoch,
 * so every mounted store-or-network query treats its cached data as stale and
 * refetches on the next read.
 */
export default function RelayErrorBoundary({
  children,
  resetKey,
  showDetailsByDefault,
}: RelayErrorBoundaryProps) {
  const environment = useRelayEnvironment();

  return (
    <ErrorBoundary
      key={resetKey}
      onReset={() => {
        commitLocalUpdate(environment, (store) => store.invalidateStore());
      }}
      fallbackRender={({ error, resetErrorBoundary }: FallbackProps) => (
        <PageErrorFallback
          error={error}
          resetErrorBoundary={resetErrorBoundary}
          showDetailsByDefault={showDetailsByDefault}
        />
      )}
    >
      {children}
    </ErrorBoundary>
  );
}

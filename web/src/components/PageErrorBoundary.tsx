import type { ReactNode } from "react";
import { ErrorBoundary, type FallbackProps } from "react-error-boundary";
import PageErrorFallback from "./PageErrorFallback";
import { useCaughtErrorMessage } from "../lib/useCaughtErrorMessage";

interface PageErrorBoundaryProps {
  children: ReactNode;
  /** Remounts the boundary (clearing its caught error) when this value changes. */
  resetKey?: string | number;
  showDetailsByDefault?: boolean;
}

/**
 * A plain `ErrorBoundary` + `PageErrorFallback` pairing for use outside a
 * `RelayEnvironmentProvider` (see `RelayErrorBoundary` for use inside one,
 * which additionally makes "Try again" refetch). "Try again" here is a bare
 * `resetErrorBoundary` — fine for the routes this wraps (home, the app root,
 * the pre-login admin shell), which don't hold Relay-fetched data that a
 * reset would need to invalidate.
 *
 * Resolves the real error message via `useCaughtErrorMessage`'s `onError`
 * capture rather than a render-phase read — see that hook for why.
 */
export default function PageErrorBoundary({
  children,
  resetKey,
  showDetailsByDefault,
}: PageErrorBoundaryProps) {
  const { message, onError, reset } = useCaughtErrorMessage();

  return (
    <ErrorBoundary
      key={resetKey}
      onError={onError}
      onReset={reset}
      fallbackRender={({ error, resetErrorBoundary }: FallbackProps) => (
        <PageErrorFallback
          error={error}
          resetErrorBoundary={resetErrorBoundary}
          showDetailsByDefault={showDetailsByDefault}
          message={message ?? undefined}
        />
      )}
    >
      {children}
    </ErrorBoundary>
  );
}

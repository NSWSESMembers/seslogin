import { useState } from "react";
import type { FallbackProps } from "react-error-boundary";
import { Button } from "./ui/Button";

interface PageErrorFallbackProps extends FallbackProps {
  showDetailsByDefault?: boolean;
  /**
   * Show "Reload page" (a real `window.location.reload()`) instead of "Try
   * again" (`resetErrorBoundary`). Use this wherever the boundary can't
   * guarantee the retry actually refetches — see RelayErrorBoundary's
   * `canRetry` — since a full reload always recovers, but a boundary reset
   * can silently redisplay the exact same error.
   */
  reloadInstead?: boolean;
}

export default function PageErrorFallback({
  error,
  resetErrorBoundary,
  showDetailsByDefault = false,
  reloadInstead = false,
}: PageErrorFallbackProps) {
  const [showDetails, setShowDetails] = useState(showDetailsByDefault);
  const message = error instanceof Error ? error.message : String(error);

  return (
    <div role="alert" className="p-6 text-center">
      <p>Something went wrong</p>
      {showDetails ? <pre className="text-red-600">{message}</pre> : null}
      {reloadInstead ? (
        <Button onClick={() => window.location.reload()}>Reload page</Button>
      ) : (
        <Button onClick={resetErrorBoundary}>Try again</Button>
      )}
      <Button className="ml-2" onClick={() => setShowDetails((prev) => !prev)}>
        {showDetails ? "Hide details" : "Show details"}
      </Button>
    </div>
  );
}

import { useState } from "react";
import type { FallbackProps } from "react-error-boundary";
import { Button } from "./ui/Button";

export default function PageErrorFallback({
  error,
  resetErrorBoundary,
  showDetailsByDefault = false,
}: FallbackProps & { showDetailsByDefault?: boolean }) {
  const [showDetails, setShowDetails] = useState(showDetailsByDefault);
  const message = error instanceof Error ? error.message : String(error);

  return (
    <div role="alert" className="p-6 text-center">
      <p>Something went wrong</p>
      {showDetails ? <pre className="text-red-600">{message}</pre> : null}
      <Button onClick={resetErrorBoundary}>Try again</Button>
      <Button className="ml-2" onClick={() => setShowDetails((prev) => !prev)}>
        {showDetails ? "Hide details" : "Show details"}
      </Button>
    </div>
  );
}

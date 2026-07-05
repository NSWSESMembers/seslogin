import Scan from "./pages/Scan";
import KioskEnvironment from "./components/KioskEnvironment";
import LoadingIndicator from "../components/LoadingIndicator";
import PageErrorFallback from "../components/PageErrorFallback";
import { Suspense } from "react";
import { ErrorBoundary } from "react-error-boundary";
import { useKioskSession } from "./components/useKioskSession";
import Status from "./pages/Status";
import { useParams } from "react-router";

export default function KioskMain() {
  const params = useParams();
  const profile = params.profile || "default";
  console.log("[KioskMain] render");
  return (
    <Suspense fallback={<LoadingIndicator />}>
      <KioskEnvironment profile={profile}>
        <ErrorBoundary FallbackComponent={PageErrorFallback}>
          <Suspense fallback={<LoadingIndicator />}>
            <Router />
          </Suspense>
        </ErrorBoundary>
      </KioskEnvironment>
    </Suspense>
  );
}

function Router() {
  const session = useKioskSession();
  if (session?.config?.status) {
    return <Status />;
  }
  return <Scan />;
}

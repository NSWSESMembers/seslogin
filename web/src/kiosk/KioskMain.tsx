import Scan from "./pages/Scan";
import KioskEnvironment from "./components/KioskEnvironment";
import LoadingIndicator from "../components/LoadingIndicator";
import PageErrorFallback from "../components/PageErrorFallback";
import { Suspense, useEffect } from "react";
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

  // The kiosk pins its theme explicitly and ignores the device's OS setting: it
  // is light by default and only goes dark when a truthy `dark` key is set in its
  // session config. We stamp `data-theme` on <html> so the tokens in app.css take
  // over the whole document, including the body background behind the kiosk view.
  const wantsDark = !!session?.config?.dark;
  useEffect(() => {
    const root = document.documentElement;
    const previous = root.getAttribute("data-theme");
    root.setAttribute("data-theme", wantsDark ? "dark" : "light");
    return () => {
      if (previous === null) {
        root.removeAttribute("data-theme");
      } else {
        root.setAttribute("data-theme", previous);
      }
    };
  }, [wantsDark]);

  if (session?.config?.status) {
    return <Status />;
  }
  return <Scan />;
}

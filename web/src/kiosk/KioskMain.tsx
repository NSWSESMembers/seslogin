import Scan from "./pages/Scan";
import KioskEnvironment from "./components/KioskEnvironment";
import LoadingIndicator from "../components/LoadingIndicator";
import PageErrorFallback from "../components/PageErrorFallback";
import { Suspense, useEffect } from "react";
import { ErrorBoundary } from "react-error-boundary";
import { useKioskSession } from "./components/useKioskSession";
import type { JsonValue } from "./components/KioskSessionContext";
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

/**
 * Maps the session config's `theme` key to the `data-theme` value to pin on
 * <html>, or `null` to leave it unpinned and follow the browser's
 * `prefers-color-scheme`. Anything other than `"dark"` or `"auto"` — including an
 * omitted key — pins light.
 */
function themeFromConfig(
  theme: JsonValue | undefined,
): "dark" | "light" | null {
  if (theme === "auto") {
    return null;
  }
  return theme === "dark" ? "dark" : "light";
}

function Router() {
  const session = useKioskSession();

  // The kiosk normally pins its theme explicitly and ignores the device's OS
  // setting: its session config's `theme` key is `"light"` by default (also when
  // omitted) and `"dark"` goes dark. The exception is `"auto"`, which leaves the
  // theme unpinned so the browser's `prefers-color-scheme` decides — this will
  // become the default later, but not yet. We stamp `data-theme` on <html> so the
  // tokens in app.css take over the whole document, including the body background
  // behind the kiosk view.
  const theme = themeFromConfig(session?.config?.theme);
  useEffect(() => {
    const root = document.documentElement;
    const previous = root.getAttribute("data-theme");
    if (theme === null) {
      root.removeAttribute("data-theme");
    } else {
      root.setAttribute("data-theme", theme);
    }
    return () => {
      if (previous === null) {
        root.removeAttribute("data-theme");
      } else {
        root.setAttribute("data-theme", previous);
      }
    };
  }, [theme]);

  if (session?.config?.status) {
    return <Status />;
  }
  return <Scan />;
}

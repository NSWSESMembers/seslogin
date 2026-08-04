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
 * Maps the session config's `dark` key to the `data-theme` value to pin on
 * <html>, or `null` to leave it unpinned and follow the browser's
 * `prefers-color-scheme`.
 */
function themeFromConfig(dark: JsonValue | undefined): "dark" | "light" | null {
  if (dark === "auto") {
    return null;
  }
  return dark ? "dark" : "light";
}

function Router() {
  const session = useKioskSession();

  // The kiosk normally pins its theme explicitly and ignores the device's OS
  // setting: it is light by default and goes dark when a truthy `dark` key is set
  // in its session config. The exception is `dark: "auto"`, which leaves the theme
  // unpinned so the browser's `prefers-color-scheme` decides. We stamp
  // `data-theme` on <html> so the tokens in app.css take over the whole document,
  // including the body background behind the kiosk view.
  const theme = themeFromConfig(session?.config?.dark);
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

import Scan from "./pages/Scan";
import KioskEnvironment from "./components/KioskEnvironment";
import LoadingIndicator from "../components/LoadingIndicator";
import RelayErrorBoundary from "../components/RelayErrorBoundary";
import { Suspense, useEffect } from "react";
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
        {/* canRetry: the two useLazyLoadQuery call sites reachable here —
            Status and ScanGuestDialog's GuestList — both thread
            useRelayRetryFetchKey() into their query. */}
        <RelayErrorBoundary canRetry>
          <Suspense fallback={<LoadingIndicator />}>
            <Router />
          </Suspense>
        </RelayErrorBoundary>
      </KioskEnvironment>
    </Suspense>
  );
}

/**
 * Maps the session config's `theme` key to the `data-theme` value to pin on
 * <html>, or `null` to leave it unpinned and follow the browser's
 * `prefers-color-scheme`. An omitted key behaves the same as `"auto"`; any other
 * invalid value pins light.
 */
function themeFromConfig(
  theme: JsonValue | undefined,
): "dark" | "light" | null {
  if (theme === undefined || theme === "auto") {
    return null;
  }
  return theme === "dark" ? "dark" : "light";
}

function Router() {
  const session = useKioskSession();

  // The kiosk's session config `theme` key defaults to `"auto"` (also when
  // omitted), which leaves the theme unpinned so the browser's
  // `prefers-color-scheme` decides. `"light"` and `"dark"` pin the theme instead,
  // ignoring the device's OS setting. We stamp `data-theme` on <html> so the
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

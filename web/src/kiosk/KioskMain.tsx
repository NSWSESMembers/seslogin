import Scan from "./pages/Scan";
import KioskEnvironment from "./components/KioskEnvironment";
import LoadingIndicator from "../components/LoadingIndicator";
import RelayErrorBoundary from "../components/RelayErrorBoundary";
import { Suspense, useEffect } from "react";
import { useKioskSession } from "./components/useKioskSession";
import type { JsonValue } from "./components/KioskSessionContext";
import Status from "./pages/Status";
import { useParams } from "react-router";
import { setKioskProfile, setContactFailureSource } from "../lib/clientInfo";
import { getKioskServerStatus } from "./lib/kioskServerStatus";
import { startWakeLock } from "./lib/wakeLock";

// Only the kiosk polls the server on a timer, so only the kiosk has a failure count
// worth reporting. Registered at module scope so it is in place before the first
// request, and so it doesn't get re-assigned on every render.
setContactFailureSource(() => getKioskServerStatus().failureCount);

export default function KioskMain() {
  const params = useParams();
  const profile = params.profile || "default";
  // Registered eagerly during render rather than in an effect: the enrollment and
  // session-refresh requests can go out before effects have run, and a snapshot
  // missing the profile is exactly the one that would confuse whoever is debugging
  // a device running several kiosk identities.
  setKioskProfile(profile);
  console.log("[KioskMain] render");

  // Keep the status bar black whatever the light/dark setting when the kiosk runs
  // from an iPad's home screen. iPadOS no longer treats index.html's
  // `apple-mobile-web-app-status-bar-style: black` as a fixed colour: it follows
  // the system appearance instead. A `theme-color` with no `media` query takes
  // precedence. Set here rather than in index.html so the homepage and admin keep
  // Safari's default toolbar colour.
  useEffect(() => {
    const meta = document.createElement("meta");
    meta.name = "theme-color";
    meta.content = "#000000";
    document.head.appendChild(meta);
    return () => meta.remove();
  }, []);

  return (
    <Suspense fallback={<LoadingIndicator />}>
      <KioskEnvironment profile={profile}>
        {/* No useLazyLoadQuery call sites remain under here — Status,
            ScanGuestDialog's GuestList, ScanStatusDialog and
            ScanSignedInPanel all read from LivePeriodsProvider now, which
            manages its own loading/error/retry state rather than suspending
            or throwing. So canRetry stays off: "Try again" resetting the
            store's invalidation epoch and bumping a Relay fetchKey (see
            RelayErrorBoundary's doc comment) wouldn't fix whatever else
            might throw here, and "Reload page" is the honest fallback. */}
        <RelayErrorBoundary>
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

  // Both kiosk faces are meant to sit unattended on a wall or a desk, so neither
  // should let the device dim and auto-lock.
  useEffect(() => startWakeLock(), []);

  if (session?.config?.status) {
    return <Status />;
  }
  return <Scan />;
}

import { useCallback, useMemo, useState, type ReactNode } from "react";
import KioskKeyRelayEnvironment from "./KioskKeyRelayEnvironment";
import { KioskEnvironmentContext } from "./KioskEnvironmentContext";
import { KioskSessionProvider } from "./KioskSessionProvider";
import KioskEnrollment from "./KioskEnrollment";
import { markKeyEnrolled, readAppSettings } from "../lib/kioskSettings";

/**
 * - `enrolling`: showing the QR code, waiting for an admin to enroll this key.
 * - `authed-key`: authenticated by signing each request with the enrolled key.
 */
type KioskAuthState = "enrolling" | "authed-key";

export default function KioskEnvironment({
  profile,
  children,
}: {
  profile: string;
  children: ReactNode;
}) {
  // Read the persisted settings once on mount. Later `profile` changes intentionally
  // don't re-seed the state below.
  const [authState, setAuthState] = useState<KioskAuthState>(() =>
    // Optimistic: assume the enrolled key still works; the first request 401s into
    // `enrolling` if the session was disabled.
    readAppSettings(profile).authMode === "key" ? "authed-key" : "enrolling",
  );

  const onEnrolled = useCallback(() => {
    markKeyEnrolled(profile);
    setAuthState("authed-key");
  }, [profile]);

  const contextValue = useMemo(
    () => ({ profile, onKeyEnrolled: onEnrolled }),
    [profile, onEnrolled],
  );

  // A 401: the session was disabled. Revert to enrollment but keep the key — it
  // re-submits the same public key so an admin can re-enroll it into a fresh session.
  const onKeyUnauthorized = useCallback(() => {
    setAuthState("enrolling");
  }, []);

  const body =
    authState === "authed-key" ? (
      <KioskKeyRelayEnvironment
        profile={profile}
        onUnauthorized={onKeyUnauthorized}
      >
        <KioskSessionProvider>{children}</KioskSessionProvider>
      </KioskKeyRelayEnvironment>
    ) : (
      <KioskEnrollment profile={profile} onEnrolled={onEnrolled} />
    );

  return (
    <KioskEnvironmentContext.Provider value={contextValue}>
      {body}
    </KioskEnvironmentContext.Provider>
  );
}

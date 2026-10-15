import { useCallback, useMemo, useRef, useState, type ReactNode } from "react";
import KioskRelayEnvironment from "./KioskRelayEnvironment";
import KioskKeyRelayEnvironment from "./KioskKeyRelayEnvironment";
import { KioskEnvironmentContext } from "./KioskEnvironmentContext";
import { KioskSessionProvider } from "./KioskSessionProvider";
import KioskEnrollment from "./KioskEnrollment";
import { KioskKeyConversion } from "./KioskKeyConversion";
import {
  markKeyEnrolled,
  readAppSettings,
  writeAppSettings,
  type AppSettingsStorage,
} from "../lib/kioskSettings";

/**
 * - `enrolling`: default; showing the QR code, waiting for an admin to enroll this key.
 * - `authed-jwt`: authenticated with a JWT from the legacy setup-code flow. New kiosks
 *   can no longer get one; this only keeps already-enrolled ones working until
 *   `KioskKeyConversion` moves them onto their key.
 * - `authed-key`: authenticated by signing each request with the enrolled key.
 */
type KioskAuthState = "enrolling" | "authed-jwt" | "authed-key";

export default function KioskEnvironment({
  profile,
  children,
}: {
  profile: string;
  children: ReactNode;
}) {
  // Lazy useState initializer: read the persisted settings once on mount. Later
  // `profile` changes intentionally don't re-seed the ref/state below.
  const [initialSettings] = useState<AppSettingsStorage>(() =>
    readAppSettings(profile),
  );
  const scanAuthTokenRef = useRef<string | null>(
    typeof initialSettings.scanAuthToken === "string"
      ? initialSettings.scanAuthToken
      : null,
  );
  const [authState, setAuthState] = useState<KioskAuthState>(() => {
    if (scanAuthTokenRef.current != null) return "authed-jwt";
    // Optimistic: assume the enrolled key still works; the first request 401s into
    // `enrolling` if the session was disabled.
    if (initialSettings.authMode === "key") return "authed-key";
    return "enrolling";
  });

  const getToken = useCallback(() => scanAuthTokenRef.current, []);

  const setToken = useCallback(
    (token: string) => {
      const issuedAt = Date.now();
      const currentSettings = readAppSettings(profile);

      writeAppSettings(profile, {
        ...currentSettings,
        scanAuthToken: token,
        scanAuthTokenIssuedAt: issuedAt,
        authMode: "jwt",
      });

      scanAuthTokenRef.current = token;
      setAuthState("authed-jwt");
    },
    [profile],
  );

  const clearToken = useCallback(() => {
    const currentSettings = readAppSettings(profile);

    writeAppSettings(profile, {
      ...currentSettings,
      scanAuthToken: null,
      scanAuthTokenIssuedAt: null,
    });

    scanAuthTokenRef.current = null;
  }, [profile]);

  const onEnrolled = useCallback(() => {
    markKeyEnrolled(profile);
    scanAuthTokenRef.current = null;
    setAuthState("authed-key");
  }, [profile]);

  const contextValue = useMemo(
    () => ({
      profile,
      authMode:
        authState === "authed-key" ? ("key" as const) : ("jwt" as const),
      onKeyEnrolled: onEnrolled,
    }),
    [profile, authState, onEnrolled],
  );

  // A legacy JWT kiosk that loses its token can only come back by enrolling its key.
  const onJwtUnauthorized = useCallback(() => {
    clearToken();
    setAuthState("enrolling");
  }, [clearToken]);

  // Key mode 401: session was disabled. Revert to enrollment but keep the key — it
  // re-submits the same public key so an admin can re-enroll it into a fresh session.
  const onKeyUnauthorized = useCallback(() => {
    setAuthState("enrolling");
  }, []);

  let body: ReactNode;
  switch (authState) {
    case "enrolling":
      body = <KioskEnrollment profile={profile} onEnrolled={onEnrolled} />;
      break;
    case "authed-jwt":
      body = (
        <KioskRelayEnvironment
          getToken={getToken}
          onUnauthorized={onJwtUnauthorized}
        >
          <KioskSessionProvider setToken={setToken}>
            <KioskKeyConversion profile={profile} />
            {children}
          </KioskSessionProvider>
        </KioskRelayEnvironment>
      );
      break;
    case "authed-key":
      body = (
        <KioskKeyRelayEnvironment
          profile={profile}
          onUnauthorized={onKeyUnauthorized}
        >
          <KioskSessionProvider
            setToken={setToken}
            persistRefreshedToken={false}
          >
            {children}
          </KioskSessionProvider>
        </KioskKeyRelayEnvironment>
      );
      break;
  }

  return (
    <KioskEnvironmentContext.Provider value={contextValue}>
      {body}
    </KioskEnvironmentContext.Provider>
  );
}

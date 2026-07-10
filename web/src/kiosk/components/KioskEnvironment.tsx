import { useCallback, useMemo, useRef, useState, type ReactNode } from "react";
import KioskRelayEnvironment from "./KioskRelayEnvironment";
import KioskKeyRelayEnvironment from "./KioskKeyRelayEnvironment";
import { KioskEnvironmentContext } from "./KioskEnvironmentContext";
import { KioskSessionProvider } from "./KioskSessionProvider";
import KioskSetup from "../pages/KioskSetup";
import KioskEnrollment from "./KioskEnrollment";

const OLD_SETTINGS_KEY = "appSettings";
const SETTINGS_PREFIX = "kiosk_";

type AppSettingsStorage = {
  scanAuthToken?: string | null;
  scanAuthTokenIssuedAt?: number | null;
  /** Which auth flow this kiosk last used: legacy 6-digit JWT, or public-key signing. */
  authMode?: "jwt" | "key" | null;
  [key: string]: unknown;
};

/**
 * - `setup-code`: default; the 6-digit code entry screen (with a QR-enroll option).
 * - `enrolling`: showing the QR code, waiting for an admin to enroll this key.
 * - `authed-jwt`: authenticated via the legacy code→JWT flow.
 * - `authed-key`: authenticated by signing each request with the enrolled key.
 */
type KioskAuthState = "setup-code" | "enrolling" | "authed-jwt" | "authed-key";

function readAppSettings(profile: string): AppSettingsStorage {
  let settingsJson = localStorage.getItem(`${SETTINGS_PREFIX}${profile}`);
  if (settingsJson == null) {
    settingsJson = localStorage.getItem(OLD_SETTINGS_KEY);
    if (settingsJson != null) {
      localStorage.removeItem(OLD_SETTINGS_KEY);
      localStorage.setItem(`${SETTINGS_PREFIX}${profile}`, settingsJson);
    } else {
      return {};
    }
  }

  try {
    const parsed = JSON.parse(settingsJson);
    if (parsed != null && typeof parsed === "object") {
      return parsed as AppSettingsStorage;
    }
  } catch (error) {
    console.error("Failed to parse app settings from localStorage", error);
  }

  return {};
}

function writeAppSettings(profile: string, settings: AppSettingsStorage) {
  localStorage.setItem(
    `${SETTINGS_PREFIX}${profile}`,
    JSON.stringify(settings),
  );
}

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
    return "setup-code";
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

  const contextValue = useMemo(
    () => ({
      setToken,
    }),
    [setToken],
  );

  const onJwtUnauthorized = useCallback(() => {
    clearToken();
    setAuthState("setup-code");
  }, [clearToken]);

  // Key mode 401: session was disabled. Revert to enrollment but keep the key — it
  // re-submits the same public key so an admin can re-enroll it into a fresh session.
  const onKeyUnauthorized = useCallback(() => {
    setAuthState("enrolling");
  }, []);

  const onEnrolled = useCallback(() => {
    const currentSettings = readAppSettings(profile);
    writeAppSettings(profile, {
      ...currentSettings,
      authMode: "key",
      scanAuthToken: null,
      scanAuthTokenIssuedAt: null,
    });
    scanAuthTokenRef.current = null;
    setAuthState("authed-key");
  }, [profile]);

  const startEnrollment = useCallback(() => setAuthState("enrolling"), []);
  const useCodeInstead = useCallback(() => setAuthState("setup-code"), []);

  let body: ReactNode;
  switch (authState) {
    case "setup-code":
      body = <KioskSetup onEnrollWithQr={startEnrollment} />;
      break;
    case "enrolling":
      body = (
        <KioskEnrollment
          profile={profile}
          onEnrolled={onEnrolled}
          onUseCodeInstead={useCodeInstead}
        />
      );
      break;
    case "authed-jwt":
      body = (
        <KioskRelayEnvironment
          getToken={getToken}
          onUnauthorized={onJwtUnauthorized}
        >
          <KioskSessionProvider setToken={setToken}>
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

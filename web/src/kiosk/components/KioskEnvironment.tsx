import { useCallback, useMemo, useRef, useState, type ReactNode } from "react";
import KioskRelayEnvironment from "./KioskRelayEnvironment";
import { KioskEnvironmentContext } from "./KioskEnvironmentContext";
import { KioskSessionProvider } from "./KioskSessionProvider";
import KioskSetup from "../pages/KioskSetup";

const OLD_SETTINGS_KEY = "appSettings";
const SETTINGS_PREFIX = "kiosk_";

type AppSettingsStorage = {
  scanAuthToken?: string | null;
  scanAuthTokenIssuedAt?: number | null;
  [key: string]: unknown;
};

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
  // Lazy useState initializer: read the persisted token once on mount. Later
  // `profile` changes intentionally don't re-seed the ref/state below.
  const [initialScanAuthToken] = useState<string | null>(() => {
    const settings = readAppSettings(profile);
    return typeof settings.scanAuthToken === "string"
      ? settings.scanAuthToken
      : null;
  });
  const scanAuthTokenRef = useRef<string | null>(initialScanAuthToken);
  const [isUnauthorized, setIsUnauthorized] = useState(
    initialScanAuthToken == null,
  );

  const getToken = useCallback(() => scanAuthTokenRef.current, []);

  const setToken = useCallback(
    (token: string) => {
      const issuedAt = Date.now();
      const currentSettings = readAppSettings(profile);

      writeAppSettings(profile, {
        ...currentSettings,
        scanAuthToken: token,
        scanAuthTokenIssuedAt: issuedAt,
      });

      scanAuthTokenRef.current = token;
      setIsUnauthorized(false);
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

  const onUnauthorized = useCallback(() => {
    clearToken();
    setIsUnauthorized(true);
  }, [clearToken]);

  if (isUnauthorized) {
    return (
      <KioskEnvironmentContext.Provider value={contextValue}>
        <KioskSetup />
      </KioskEnvironmentContext.Provider>
    );
  }

  return (
    <KioskEnvironmentContext.Provider value={contextValue}>
      <KioskRelayEnvironment
        getToken={getToken}
        onUnauthorized={onUnauthorized}
      >
        <KioskSessionProvider setToken={setToken}>
          {children}
        </KioskSessionProvider>
      </KioskRelayEnvironment>
    </KioskEnvironmentContext.Provider>
  );
}

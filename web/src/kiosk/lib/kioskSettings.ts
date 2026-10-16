const OLD_SETTINGS_KEY = "appSettings";
const SETTINGS_PREFIX = "kiosk_";

export type AppSettingsStorage = {
  scanAuthToken?: string | null;
  scanAuthTokenIssuedAt?: number | null;
  /**
   * Which auth flow this kiosk last used: legacy JWT (from a since-removed 6-digit setup
   * code, still honoured for kiosks enrolled that way), or public-key signing.
   */
  authMode?: "jwt" | "key" | null;
  [key: string]: unknown;
};

/** The persisted settings for a kiosk profile (`/kiosk/:profile`). */
export function readAppSettings(profile: string): AppSettingsStorage {
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

export function writeAppSettings(
  profile: string,
  settings: AppSettingsStorage,
) {
  localStorage.setItem(
    `${SETTINGS_PREFIX}${profile}`,
    JSON.stringify(settings),
  );
}

/**
 * Record that this kiosk now authenticates with its enrolled key, dropping any stored
 * JWT. The next load starts straight in key mode.
 */
export function markKeyEnrolled(profile: string) {
  writeAppSettings(profile, {
    ...readAppSettings(profile),
    authMode: "key",
    scanAuthToken: null,
    scanAuthTokenIssuedAt: null,
  });
}

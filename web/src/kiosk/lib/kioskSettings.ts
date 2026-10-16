const OLD_SETTINGS_KEY = "appSettings";
const SETTINGS_PREFIX = "kiosk_";

/**
 * Keys a kiosk enrolled with the removed 6-digit setup code stored its session JWT
 * under. JWTs are refused now, so they are dropped wherever the settings are read.
 */
const LEGACY_JWT_KEYS = ["scanAuthToken", "scanAuthTokenIssuedAt"];

export type AppSettingsStorage = {
  /** `"key"` once this kiosk's key has been enrolled (anything else: not yet). */
  authMode?: string | null;
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
      const settings = parsed as AppSettingsStorage;
      if (LEGACY_JWT_KEYS.some((key) => key in settings)) {
        for (const key of LEGACY_JWT_KEYS) {
          delete settings[key];
        }
        writeAppSettings(profile, settings);
      }
      return settings;
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
 * Record that this kiosk now authenticates with its enrolled key. The next load starts
 * straight in key mode.
 */
export function markKeyEnrolled(profile: string) {
  writeAppSettings(profile, { ...readAppSettings(profile), authMode: "key" });
}

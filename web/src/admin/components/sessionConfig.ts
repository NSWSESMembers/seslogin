/**
 * Config a brand-new kiosk starts with. Only the theme is set: new kiosks follow
 * the device's own light/dark setting, which is also what an omitted `theme` key
 * means to the kiosk itself (see `getThemeFromConfig` in SessionForm, and
 * `themeFromConfig` in the kiosk's KioskMain).
 */
export const NEW_SESSION_CONFIG = JSON.stringify({ theme: "auto" });

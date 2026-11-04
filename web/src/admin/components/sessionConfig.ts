/**
 * Config a brand-new kiosk starts with. Only the theme is set: new kiosks follow
 * the device's own light/dark setting, which is also what an omitted `theme` key
 * means to the kiosk itself (see `getThemeFromConfig` in SessionForm, and
 * `themeFromConfig` in the kiosk's KioskMain). Confirm visitors is on too, by
 * leaving `confirmVisitors` out: only turning it off is stored.
 */
export const NEW_SESSION_CONFIG = JSON.stringify({ theme: "auto" });

import type { JsonValue } from "../components/KioskSessionContext";
import { isIPad } from "./fullscreen";

/**
 * How members enter their ID on a kiosk, chosen by the session config's
 * `interfaceMode` key (see SessionForm):
 *
 * - `"mouseKeyboard"`: no on-screen number pad; the ID comes from a barcode
 *   scanner or a physical keyboard.
 * - `"touch"`: tapping the member ID field (or the pad button beside it) opens
 *   the on-screen number pad, and the system keyboard is kept from appearing
 *   for that field.
 * - `"auto"` (also an omitted or unrecognised key): {@link detectInterfaceMode}
 *   picks one of the two from the device.
 */
export type InterfaceMode = "auto" | "mouseKeyboard" | "touch";
export type ResolvedInterfaceMode = Exclude<InterfaceMode, "auto">;

export function interfaceModeFromConfig(
  value: JsonValue | undefined,
): InterfaceMode {
  return value === "mouseKeyboard" || value === "touch" ? value : "auto";
}

/**
 * A best guess at whether this device is driven by touch. Any iPhone, iPod or
 * iPad counts — a keyboard attached to an iPad doesn't make the system keyboard
 * any less likely to cover the screen — as does any other device whose only
 * pointer is coarse (a touch-only Android tablet or Windows kiosk). A device
 * with a mouse or trackpad, touchscreen or not, is treated as mouse/keyboard.
 */
export function detectInterfaceMode(): ResolvedInterfaceMode {
  if (/iPhone|iPod/.test(navigator.userAgent) || isIPad()) {
    return "touch";
  }
  if (typeof window.matchMedia !== "function") {
    return "mouseKeyboard";
  }
  const coarse = window.matchMedia("(any-pointer: coarse)").matches;
  const fine = window.matchMedia("(any-pointer: fine)").matches;
  return coarse && !fine ? "touch" : "mouseKeyboard";
}

export function resolveInterfaceMode(
  mode: InterfaceMode,
): ResolvedInterfaceMode {
  return mode === "auto" ? detectInterfaceMode() : mode;
}

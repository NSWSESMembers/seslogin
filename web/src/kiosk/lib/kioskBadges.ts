import type { KioskSession } from "../components/KioskSessionContext";

/**
 * Whether this kiosk celebrates badges: its own `gamification` config key.
 * Admins only offer that option at a location with gamification on, and the
 * server works out badges only when the kiosk's location and the member's primary
 * location are on too, so a kiosk left switched on elsewhere simply never gets
 * any to show. Keying off the config alone keeps the kiosk in step with an
 * admin's change straight away: config is pushed, the location is not.
 */
export function kioskBadgesEnabled(
  session: KioskSession | null | undefined,
): boolean {
  return session?.config?.gamification === true;
}

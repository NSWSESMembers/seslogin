import { getCurrentEnvironment } from "../../lib/clientInfo";

/**
 * When a legacy setup-code (JWT) kiosk may convert itself to key auth.
 *
 * The rollout is staggered so a problem shows up on a couple of kiosks before it
 * reaches all of them. All times are Australia/Sydney on Saturday 2026-10-17 (AEDT,
 * UTC+11), written as absolute instants so the kiosk's own timezone setting can't
 * shift them.
 */

/** Kiosks that go first, each at a fixed time. */
const PINNED: Record<string, number> = {
  // 09:00 Sydney
  NjHCubqnmslo: Date.UTC(2026, 9, 16, 22, 0, 0),
  // 10:00 Sydney
  UL23XGaM9wD1: Date.UTC(2026, 9, 16, 23, 0, 0),
};

/** Everyone else is spread across 20:00–21:00 Sydney. */
const WINDOW_START_MS = Date.UTC(2026, 9, 17, 9, 0, 0);

/** On local dev and test.seslogin.com, any kiosk converts after this much page uptime. */
export const TEST_UPTIME_MS = 2 * 60 * 1000;
const TEST_ENVIRONMENTS = new Set(["dev", "test"]);

/** 32-bit FNV-1a: cheap, synchronous, and spreads short IDs well enough. */
function fnv1a32(text: string): number {
  let hash = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    hash ^= text.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

/** The instant (epoch ms) from which `sessionId` may convert. */
export function conversionEligibleAt(sessionId: string): number {
  const pinned = PINNED[sessionId];
  if (pinned !== undefined) {
    return pinned;
  }
  // hash / 2^32 is in [0, 1), so the offset is a whole second in [0, 3600).
  const offsetSecs = Math.floor((fnv1a32(sessionId) / 2 ** 32) * 3600);
  return WINDOW_START_MS + offsetSecs * 1000;
}

export type EligibilityOptions = {
  now?: number;
  uptimeMs?: number;
  environment?: string;
};

function pageUptimeMs(): number {
  return typeof performance !== "undefined" &&
    typeof performance.now === "function"
    ? performance.now()
    : 0;
}

export function isEligibleForKeyConversion(
  sessionId: string,
  {
    now = Date.now(),
    uptimeMs = pageUptimeMs(),
    environment = getCurrentEnvironment(),
  }: EligibilityOptions = {},
): boolean {
  if (TEST_ENVIRONMENTS.has(environment) && uptimeMs >= TEST_UPTIME_MS) {
    return true;
  }
  return now >= conversionEligibleAt(sessionId);
}

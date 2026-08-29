import { getGraphQLEndpoint } from "./api";
import { getClientUpdateState } from "./clientUpdate";
import {
  CLIENT_VERSION_HEADER,
  getCurrentClientVersion,
} from "./clientVersion";

/**
 * What this client tells the server about itself, sent as a compact JSON object in the
 * `X-Client-Info` header on every request. The server bounds and stores it on the kiosk's
 * session record, so it shows up in the admin kiosk list without the kiosk needing to be
 * reachable or physically inspected.
 *
 * The point of most of these is to answer "why is *this one* kiosk behaving differently?"
 * — a question that otherwise needs someone standing in front of it. `env` and `origin`
 * answer a sharper one: `test` and `preprod` front-ends talk to the production database,
 * so a kiosk running one of those builds is on live data and nothing else would show it.
 *
 * Everything here is best-effort. Collection is wrapped so that a browser missing an API
 * costs one field, never the request — diagnostics must not be able to break a kiosk.
 */
export const CLIENT_INFO_HEADER = "X-Client-Info";

export type ClientInfo = {
  env?: string;
  origin?: string;
  apiUrl?: string;
  profile?: string;
  screen?: string;
  displayMode?: string;
  timezone?: string;
  clockMs?: number;
  uptimeSecs?: number;
  pendingVersion?: string;
  contactFailures?: number;
};

/** Display modes a PWA can be launched in, most-installed first. */
const DISPLAY_MODES = ["standalone", "fullscreen", "minimal-ui"] as const;

/**
 * Facts only the kiosk knows about itself, registered once at startup. Module-level
 * rather than context because `fetchGraphQL` is shared with the admin app and has no
 * React tree to read from — the same reasoning as `kioskServerStatus`.
 */
let kioskProfile: string | null = null;

/** Called by the kiosk shell once its route profile is known. */
export function setKioskProfile(profile: string): void {
  kioskProfile = profile;
}

/** Reported failed server contacts, supplied by the kiosk's own poll bookkeeping. */
let contactFailureSource: (() => number) | null = null;

export function setContactFailureSource(source: () => number): void {
  contactFailureSource = source;
}

/** Test-only: drop the registrations above. */
export function resetClientInfo(): void {
  kioskProfile = null;
  contactFailureSource = null;
}

function getDisplayMode(): string | undefined {
  if (
    typeof window === "undefined" ||
    typeof window.matchMedia !== "function"
  ) {
    return undefined;
  }
  const mode = DISPLAY_MODES.find(
    (candidate) => window.matchMedia(`(display-mode: ${candidate})`).matches,
  );
  return mode ?? "browser";
}

function getScreen(): string | undefined {
  if (typeof window === "undefined") {
    return undefined;
  }
  const { innerWidth, innerHeight, devicePixelRatio } = window;
  if (!innerWidth || !innerHeight) {
    return undefined;
  }
  // Round the ratio: some browsers report it to many decimal places at odd zoom
  // levels, which would churn the stored value without telling anyone anything.
  const ratio = Math.round((devicePixelRatio || 1) * 100) / 100;
  return `${innerWidth}x${innerHeight}@${ratio}`;
}

function getTimezone(): string | undefined {
  return Intl.DateTimeFormat().resolvedOptions().timeZone || undefined;
}

function getUptimeSecs(): number | undefined {
  if (
    typeof performance === "undefined" ||
    typeof performance.now !== "function"
  ) {
    return undefined;
  }
  return Math.round(performance.now() / 1000);
}

/**
 * Which build channel this bundle was deployed as — `prod`, `preprod`, `test`, or `dev`
 * for anything built without `VITE_ENVIRONMENT` set (local dev, and any older build).
 * Also what the admin UI compares a kiosk's reported environment against.
 */
export function getCurrentEnvironment(): string {
  const env: unknown = import.meta.env.VITE_ENVIRONMENT;
  return typeof env === "string" && env.length > 0 ? env : "dev";
}

function collect(): ClientInfo {
  const info: ClientInfo = {
    env: getCurrentEnvironment(),
    origin: typeof window === "undefined" ? undefined : window.location.origin,
    apiUrl: getGraphQLEndpoint(),
    profile: kioskProfile ?? undefined,
    screen: getScreen(),
    displayMode: getDisplayMode(),
    timezone: getTimezone(),
    clockMs: Date.now(),
    uptimeSecs: getUptimeSecs(),
    pendingVersion: getClientUpdateState().pendingVersion ?? undefined,
    contactFailures: contactFailureSource?.(),
  };
  // Omit rather than send nulls: the server treats a present field as reported, and a
  // reported-but-empty field is a different claim from an unreported one.
  for (const key of Object.keys(info) as (keyof ClientInfo)[]) {
    if (info[key] == null) {
      delete info[key];
    }
  }
  return info;
}

/**
 * The `X-Client-Version` and `X-Client-Info` headers every request carries. Collection
 * failures degrade to sending less, never to throwing — a broken diagnostic must not be
 * able to stop a kiosk from signing someone in.
 */
export function clientHeaders(): Record<string, string> {
  const headers: Record<string, string> = {
    [CLIENT_VERSION_HEADER]: getCurrentClientVersion(),
  };
  try {
    headers[CLIENT_INFO_HEADER] = JSON.stringify(collect());
  } catch (error) {
    console.warn("Failed to collect client info", error);
  }
  return headers;
}

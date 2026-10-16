import { isEligibleForKeyConversion } from "./keyConversion";
import {
  buildSignedAuthHeader,
  getOrCreateKioskKey,
  type KioskKeyInfo,
} from "./kioskKey";
import { markKeyEnrolled } from "./kioskSettings";

/** How often a JWT kiosk checks whether its conversion time has come. */
export const CHECK_INTERVAL_MS = 60 * 1000;
/** How long it waits after a failed attempt before trying again. */
export const RETRY_INTERVAL_MS = 5 * 60 * 1000;

const SCHEME = "SLKey ";

export type KeyConversionDeps = {
  profile: string;
  sessionId: string;
  /** Commit `convertSessionToKey`; rejects on any error. */
  commit: (publicKey: string, proof: string) => Promise<void>;
  isEligible?: (sessionId: string) => boolean;
  getKey?: (profile: string) => Promise<KioskKeyInfo>;
  /** Called once the server holds the key and key mode is persisted. */
  onConverted?: () => void;
};

/**
 * The proof `convertSessionToKey` wants: the kiosk's ordinary request signature, over
 * `slkey-convert:<session id>` instead of a request body, minus the `SLKey ` scheme.
 */
export async function buildConversionProof(
  key: KioskKeyInfo,
  sessionId: string,
): Promise<string> {
  const header = await buildSignedAuthHeader(key, `slkey-convert:${sessionId}`);
  return header.slice(SCHEME.length);
}

/**
 * Move a legacy JWT kiosk onto key auth once its conversion time comes: attach its key
 * to its session, persist key mode, then hand over to `onConverted` (a reload, by
 * default). Any failure leaves the kiosk working on its JWT, and it tries again later.
 * Returns a stop function.
 */
export function startKeyConversion({
  profile,
  sessionId,
  commit,
  isEligible = isEligibleForKeyConversion,
  getKey = getOrCreateKioskKey,
  onConverted = () => window.location.reload(),
}: KeyConversionDeps): () => void {
  let stopped = false;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const schedule = (delayMs: number) => {
    if (!stopped) {
      timer = setTimeout(tick, delayMs);
    }
  };

  const attempt = async () => {
    const key = await getKey(profile);
    const proof = await buildConversionProof(key, sessionId);
    await commit(key.publicKeySpkiB64, proof);
  };

  async function tick() {
    if (stopped) {
      return;
    }
    if (!isEligible(sessionId)) {
      schedule(CHECK_INTERVAL_MS);
      return;
    }
    try {
      console.log(
        `[KeyConversion] converting session ${sessionId} to key auth`,
      );
      await attempt();
    } catch (err) {
      console.warn("[KeyConversion] conversion failed; staying on JWT:", err);
      schedule(RETRY_INTERVAL_MS);
      return;
    }
    if (stopped) {
      return;
    }
    markKeyEnrolled(profile);
    console.log("[KeyConversion] converted; reloading into key mode");
    onConverted();
  }

  void tick();

  return () => {
    stopped = true;
    clearTimeout(timer);
  };
}

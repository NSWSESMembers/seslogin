import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  CHECK_INTERVAL_MS,
  RETRY_INTERVAL_MS,
  startKeyConversion,
} from "./keyConversionRunner";
import { keyInfoFromKeyPair, type KioskKeyInfo } from "./kioskKey";
import { readAppSettings, writeAppSettings } from "./kioskSettings";

let key: KioskKeyInfo;

beforeEach(async () => {
  const keyPair = await crypto.subtle.generateKey(
    { name: "ECDSA", namedCurve: "P-256" },
    false,
    ["sign", "verify"],
  );
  key = await keyInfoFromKeyPair(keyPair);
  localStorage.clear();
  writeAppSettings("default", { scanAuthToken: "jwt", authMode: "jwt" });
  vi.useFakeTimers();
  vi.spyOn(console, "log").mockImplementation(() => {});
  vi.spyOn(console, "warn").mockImplementation(() => {});
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

/**
 * Wait for `check` to pass. Signing is real WebCrypto, which resolves on its own
 * schedule rather than the fake clock's, so a fixed microtask flush isn't enough.
 */
async function until(check: () => void) {
  await vi.waitFor(check, { timeout: 2000, interval: 5 });
}

function start(overrides: {
  eligible: () => boolean;
  commit?: (publicKey: string, proof: string) => Promise<void>;
}) {
  const commit = vi.fn(overrides.commit ?? (() => Promise.resolve()));
  const onConverted = vi.fn();
  const stop = startKeyConversion({
    profile: "default",
    sessionId: "Sess1",
    commit,
    isEligible: overrides.eligible,
    getKey: () => Promise.resolve(key),
    onConverted,
  });
  return { commit, onConverted, stop };
}

describe("startKeyConversion", () => {
  it("does nothing until the session is eligible", async () => {
    let eligible = false;
    const { commit, onConverted, stop } = start({ eligible: () => eligible });
    await vi.advanceTimersByTimeAsync(CHECK_INTERVAL_MS * 3);
    expect(commit).not.toHaveBeenCalled();

    eligible = true;
    await vi.advanceTimersByTimeAsync(CHECK_INTERVAL_MS);
    await until(() => expect(onConverted).toHaveBeenCalledTimes(1));
    expect(commit).toHaveBeenCalledTimes(1);
    stop();
  });

  it("sends its public key with a proof signed for this session, then switches to key mode", async () => {
    const { commit, onConverted, stop } = start({ eligible: () => true });
    await until(() => expect(onConverted).toHaveBeenCalledTimes(1));

    expect(commit).toHaveBeenCalledTimes(1);
    const [publicKey, proof] = commit.mock.calls[0];
    expect(publicKey).toBe(key.publicKeySpkiB64);
    const [fingerprint, timestamp, signature] = proof.split(".");
    expect(fingerprint).toBe(key.fingerprint);
    expect(Number(timestamp)).toBeGreaterThan(0);
    expect(signature.length).toBeGreaterThan(0);

    expect(readAppSettings("default")).toMatchObject({
      authMode: "key",
      scanAuthToken: null,
    });
    stop();
  });

  it("stays on the JWT after a failure, and retries later", async () => {
    let fail = true;
    const { commit, onConverted, stop } = start({
      eligible: () => true,
      commit: () =>
        fail ? Promise.reject(new Error("CONFLICT")) : Promise.resolve(),
    });
    await until(() => expect(console.warn).toHaveBeenCalledTimes(1));
    expect(commit).toHaveBeenCalledTimes(1);
    expect(onConverted).not.toHaveBeenCalled();
    expect(readAppSettings("default")).toMatchObject({
      authMode: "jwt",
      scanAuthToken: "jwt",
    });

    await vi.advanceTimersByTimeAsync(RETRY_INTERVAL_MS - 1);
    expect(commit).toHaveBeenCalledTimes(1);

    fail = false;
    await vi.advanceTimersByTimeAsync(1);
    await until(() => expect(onConverted).toHaveBeenCalledTimes(1));
    expect(commit).toHaveBeenCalledTimes(2);
    stop();
  });

  it("stops checking once stopped", async () => {
    const eligible = vi.fn(() => false);
    const { stop } = start({ eligible });
    await vi.advanceTimersByTimeAsync(0);
    stop();
    await vi.advanceTimersByTimeAsync(CHECK_INTERVAL_MS * 5);
    expect(eligible).toHaveBeenCalledTimes(1);
  });
});

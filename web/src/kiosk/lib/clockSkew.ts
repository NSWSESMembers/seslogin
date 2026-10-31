import { useSyncExternalStore } from "react";

/**
 * Whether the server last refused this kiosk's signed requests because the device's
 * clock is wrong. Every request is signed with a timestamp from the device's clock and
 * the server refuses one more than 5 minutes out (`CLOCK_SKEW`). A computer can start
 * up with its clock out — a flat CMOS battery, or before it has synced its time — and
 * until it's fixed, every request fails.
 *
 * That is not a revoked key, so the kiosk stays enrolled and shows a notice asking for
 * the clock to be fixed (KioskClockSkewNotice), instead of dropping to the enrollment
 * screen, where an admin would enroll it as a new kiosk. The kiosk does not correct
 * for the skew itself: the computer's clock is wrong for everything else on it too, so
 * the fix belongs there. The notice clears on the next signed request that succeeds.
 *
 * Module-level rather than in context: the requests reporting it come from the Relay
 * environment and the enrollment poll, neither of which sits under one provider.
 */
export type ClockSkew = {
  /** The server's clock minus this device's, in milliseconds, when last refused. */
  offsetMs: number;
};

/** How often a kiosk refused for its clock retries, so it recovers soon after a fix. */
export const CLOCK_SKEW_RETRY_MS = 30 * 1000;

let current: ClockSkew | null = null;
const listeners = new Set<() => void>();

function set(next: ClockSkew | null): void {
  if (next === current) {
    return;
  }
  current = next;
  listeners.forEach((listener) => listener());
}

/** Record a `CLOCK_SKEW` refusal; `serverTimeSec` is the server's clock (Unix seconds). */
export function reportClockSkew(serverTimeSec: number): void {
  const offsetMs = serverTimeSec * 1000 - Date.now();
  if (current?.offsetMs === offsetMs) {
    return;
  }
  console.warn(
    `Kiosk clock is ${Math.round(offsetMs / 1000)}s behind the server; requests are being refused`,
  );
  set({ offsetMs });
}

/** A signed request was accepted, so the clock is (now) within the server's window. */
export function clearClockSkew(): void {
  set(null);
}

export function getClockSkew(): ClockSkew | null {
  return current;
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useClockSkew(): ClockSkew | null {
  return useSyncExternalStore(subscribe, getClockSkew);
}

/**
 * How far out the device's clock is, in words: `offsetMs` is the server's clock minus
 * the device's, so a positive offset means the device is behind ("slow").
 */
export function describeClockOffset(offsetMs: number): string {
  const direction = offsetMs > 0 ? "slow" : "fast";
  const seconds = Math.abs(offsetMs) / 1000;
  const units: [number, string][] = [
    [24 * 60 * 60, "day"],
    [60 * 60, "hour"],
    [60, "minute"],
  ];
  for (const [size, name] of units) {
    if (seconds >= size) {
      const count = Math.round(seconds / size);
      return `${count} ${name}${count === 1 ? "" : "s"} ${direction}`;
    }
  }
  const count = Math.round(seconds);
  return `${count} second${count === 1 ? "" : "s"} ${direction}`;
}

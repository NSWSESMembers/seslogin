/**
 * Keeps the kiosk's screen on with the Screen Wake Lock API, so an iPad (or any other
 * tablet) left at the door doesn't dim and auto-lock between sign-ins.
 *
 * The browser drops the lock whenever the page is hidden — the screen was locked anyway,
 * the app was switched away from, the tab was backgrounded — and never gives it back by
 * itself, so we re-request it every time the page becomes visible again. A request can
 * also be refused outright (Safari has done so without a recent user gesture, and iOS
 * home-screen apps before iPadOS 18.4 refused it altogether), so we retry on the next tap
 * as well: someone signing in is exactly the gesture it may be waiting for.
 *
 * Module-level state rather than context, for the same reason as `kioskServerStatus`:
 * only the status panel reads it, via `useSyncExternalStore`.
 */

export type WakeLockState =
  /** The browser has no Screen Wake Lock API; the device's own sleep setting applies. */
  | "unsupported"
  /** Requested, answer not back yet. */
  | "pending"
  /** Held: the screen will stay on while the kiosk is visible. */
  | "held"
  /** Refused or released; retried on the next visibility change or tap. */
  | "released";

type Status = { state: WakeLockState; lastError: string | null };

let status: Status = { state: "unsupported", lastError: null };
let sentinel: WakeLockSentinel | null = null;
let requesting = false;
let stopCurrent: (() => void) | null = null;
const listeners = new Set<() => void>();

/** Replaces the status object on every change, as `useSyncExternalStore` requires. */
function setStatus(next: Status): void {
  status = next;
  listeners.forEach((listener) => listener());
}

export function getWakeLockStatus(): Status {
  return status;
}

export function subscribeWakeLock(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function request(): Promise<void> {
  if (
    requesting ||
    sentinel != null ||
    document.visibilityState !== "visible"
  ) {
    return;
  }
  requesting = true;
  setStatus({ ...status, state: "pending" });
  try {
    const lock = await navigator.wakeLock.request("screen");
    if (stopCurrent == null) {
      // Stopped while the request was in flight.
      await lock.release();
      return;
    }
    sentinel = lock;
    setStatus({ state: "held", lastError: null });
    lock.addEventListener("release", () => {
      if (sentinel === lock) {
        sentinel = null;
        setStatus({ ...status, state: "released" });
      }
    });
  } catch (error) {
    setStatus({ state: "released", lastError: describeError(error) });
  } finally {
    requesting = false;
  }
}

/**
 * Starts holding the wake lock until the returned function is called. Only one holder
 * at a time: a second call replaces the first.
 */
export function startWakeLock(): () => void {
  stopCurrent?.();
  if (typeof navigator === "undefined" || !("wakeLock" in navigator)) {
    setStatus({ state: "unsupported", lastError: null });
    return () => {};
  }

  const onChance = () => {
    void request();
  };
  document.addEventListener("visibilitychange", onChance);
  document.addEventListener("pointerdown", onChance, true);

  const stop = () => {
    if (stopCurrent !== stop) {
      return;
    }
    stopCurrent = null;
    document.removeEventListener("visibilitychange", onChance);
    document.removeEventListener("pointerdown", onChance, true);
    const lock = sentinel;
    sentinel = null;
    setStatus({ state: "released", lastError: null });
    void lock?.release().catch(() => {});
  };
  stopCurrent = stop;
  void request();
  return stop;
}

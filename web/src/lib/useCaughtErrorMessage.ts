import { useState } from "react";
import { describeCaughtError } from "./relayFieldLogger";

/**
 * Resolves the display message for an error caught by an `ErrorBoundary`, via
 * its `onError` prop rather than a render-phase read.
 *
 * `describeCaughtError` calls `takeRecentFieldErrorMessages`, which *drains* a
 * buffer as a side effect — safe to call exactly once per real catch, but not
 * safe from render-phase code: React's StrictMode deliberately double-invokes
 * things like a `useState` lazy initializer to catch exactly this kind of
 * impurity, so the first (thrown-away) call would drain the real message and
 * the second, "real" call would find the buffer already empty.
 *
 * `componentDidCatch` — what `onError` maps to — is a real lifecycle method,
 * not a render-phase function, and React guarantees it runs exactly once per
 * catch even under StrictMode. It fires after the fallback's first paint
 * (same commit, not a visible flash in practice — the message stays behind a
 * collapsed "Show details" until then), so `message` starts `null` and
 * resolves moments later; callers should fall back to the error's own message
 * until it does.
 */
export function useCaughtErrorMessage() {
  const [message, setMessage] = useState<string | null>(null);
  function onError(error: unknown) {
    setMessage(describeCaughtError(error));
  }
  // Call from onReset: without this, a retry that throws again shows the
  // previous error's resolved message for one frame, paired with the new
  // error object, until onError re-fires for it.
  function reset() {
    setMessage(null);
  }
  return { message, onError, reset };
}

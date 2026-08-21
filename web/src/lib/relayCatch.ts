import { isValueResult, type Result } from "relay-runtime";

/**
 * Unwraps a field read with `@catch` (default `to: RESULT`), throwing if the field
 * itself failed to resolve. Never treat a failed relation as merely absent — that
 * would misrepresent real data (a real member rendering as a guest, a real category
 * as none) — so this throws rather than falling back to null, letting a surrounding
 * error boundary (e.g. one scoped to a single table row) degrade just that scope
 * instead of silently showing wrong data.
 */
export function unwrapCatch<T>(result: Result<T, unknown>): T {
  if (!isValueResult(result)) {
    throw new Error("Failed to resolve a @catch'd relation");
  }
  return result.value;
}

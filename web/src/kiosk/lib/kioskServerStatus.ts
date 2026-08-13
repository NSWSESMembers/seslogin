/**
 * Last-known result of the kiosk's periodic session refresh, kept as a module-level
 * snapshot so the status panel can report whether this kiosk is still reaching the
 * server. Deliberately outside `KioskSessionContext`: these values change on every
 * poll, and putting them in context would re-render the whole kiosk tree on a timer.
 */

export type KioskServerStatus = {
  /** `Date.now()` of the last refresh that came back successfully. */
  lastSuccessAt: number | null;
  /** `Date.now()` of the most recent refresh attempt, successful or not. */
  lastAttemptAt: number | null;
  /** `Date.now()` of the last failed refresh, cleared on the next success. */
  lastFailureAt: number | null;
  /** Message from that failure, cleared on the next success. */
  lastErrorMessage: string | null;
  /** Unix seconds; when the enrolled key expires. Null for code/JWT kiosks. */
  keyExpiresAt: number | null;
};

let status: KioskServerStatus = {
  lastSuccessAt: null,
  lastAttemptAt: null,
  lastFailureAt: null,
  lastErrorMessage: null,
  keyExpiresAt: null,
};

function describeError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "string") {
    return error;
  }
  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

export function recordServerContactSuccess(
  keyExpiresAt: number | null = null,
): void {
  const now = Date.now();
  status = {
    lastSuccessAt: now,
    lastAttemptAt: now,
    lastFailureAt: null,
    lastErrorMessage: null,
    keyExpiresAt,
  };
}

export function recordServerContactFailure(error: unknown): void {
  const now = Date.now();
  status = {
    ...status,
    lastAttemptAt: now,
    lastFailureAt: now,
    lastErrorMessage: describeError(error),
  };
}

export function getKioskServerStatus(): KioskServerStatus {
  return status;
}

/** Test-only: drop everything recorded so far. */
export function resetKioskServerStatus(): void {
  status = {
    lastSuccessAt: null,
    lastAttemptAt: null,
    lastFailureAt: null,
    lastErrorMessage: null,
    keyExpiresAt: null,
  };
}

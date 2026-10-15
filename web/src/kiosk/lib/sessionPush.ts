import type {
  JsonValue,
  KioskSession,
} from "../components/KioskSessionContext";

/** An admin edited this session; carries what the kiosk shows from it. */
export const EVENT_SESSION_UPDATED = "session.updated";
/** Refetch this session: its key was released to a newly enrolled session, it
 * was deleted, or the edit was too large to send. */
export const EVENT_SESSION_CHANGED = "session.changed";

export interface SessionUpdatedPayload {
  sessionId: string;
  version: number;
  name: string;
  config: { [key: string]: JsonValue };
}

export interface SessionChangedPayload {
  sessionId: string;
  version: number;
}

/** Validates a `session.updated` message's decoded `data`, like
 * `parseOpenedPayload` does for periods. `null` for anything malformed. */
export function parseSessionUpdated(
  data: unknown,
): SessionUpdatedPayload | null {
  if (typeof data !== "object" || data === null) return null;
  const d = data as Record<string, unknown>;
  if (
    typeof d.sessionId === "string" &&
    typeof d.version === "number" &&
    typeof d.name === "string" &&
    typeof d.config === "object" &&
    d.config !== null &&
    !Array.isArray(d.config)
  ) {
    return {
      sessionId: d.sessionId,
      version: d.version,
      name: d.name,
      config: d.config as { [key: string]: JsonValue },
    };
  }
  return null;
}

/** Same validation, for `session.changed`. */
export function parseSessionChanged(
  data: unknown,
): SessionChangedPayload | null {
  if (typeof data !== "object" || data === null) return null;
  const d = data as Record<string, unknown>;
  if (typeof d.sessionId === "string" && typeof d.version === "number") {
    return { sessionId: d.sessionId, version: d.version };
  }
  return null;
}

/** Whether a push about `sessionId` at `version` is news to a kiosk holding
 * `current`: it must be about this session, and strictly newer. A redelivered
 * or out-of-order message (Ably is at-least-once) is ignored. */
export function isNewerPush(
  current: KioskSession | null,
  sessionId: string,
  version: number,
): boolean {
  return (
    current != null && current.id === sessionId && version > current.version
  );
}

/** Applies a `session.updated` push, or returns `current` unchanged when it
 * isn't news (see `isNewerPush`). */
export function applySessionUpdated(
  current: KioskSession | null,
  push: SessionUpdatedPayload,
): KioskSession | null {
  if (!isNewerPush(current, push.sessionId, push.version)) return current;
  return {
    ...current!,
    name: push.name,
    config: push.config,
    version: push.version,
  };
}

/** Whether a refetched session should replace `current`. A different session
 * (the kiosk was re-enrolled), or none at all, always does; the same session
 * only at an equal or newer version, so a read that lags a push already
 * applied can't roll it back. */
export function acceptFetchedSession(
  current: KioskSession | null,
  fetched: KioskSession | null,
): boolean {
  if (current == null || fetched == null || current.id !== fetched.id) {
    return true;
  }
  return fetched.version >= current.version;
}

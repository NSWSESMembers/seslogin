import type { StatusPeriod } from "../components/StatusCurrentDisplay";

/** Ably event names published by the server — see api/src/realtime.rs. */
export const EVENT_PERIOD_OPENED = "period.opened";
export const EVENT_PERIOD_CLOSED = "period.closed";

/** Wire shape of a `period.opened` Ably message (and of a kiosk's own optimistic
 * apply after `scanRegister2`/`scanGuestSignIn` succeeds). */
export interface PeriodOpenedPayload {
  periodId: string;
  version: number;
  /** Display name: the guest's name, or "First Last" for a member. Not yet
   * suffixed " (Guest)" — that's applied at selection time. */
  name: string;
  guest: boolean;
  startTime: number;
}

/** Wire shape of a `period.closed` Ably message (and of a kiosk's own optimistic
 * apply after `scanSignOut`/`scanGuestSignOut` succeeds). `endTime` and `deleted`
 * aren't read by the reducer below — a closed period never shows on a kiosk list
 * regardless of how it closed — but are kept on the type so a caller building one
 * from a mutation result or an Ably message doesn't have to drop them. */
export interface PeriodClosedPayload {
  periodId: string;
  version: number;
  name: string;
  guest: boolean;
  startTime: number;
  endTime: number | null;
  deleted: boolean;
}

/** One row of the `periods(onlyActive: true)` snapshot query, the subset this
 * module needs. */
export interface SnapshotEntry {
  id: string;
  version: number;
  name: string;
  guest: boolean;
  startTime: number;
}

interface LiveEntry {
  version: number;
  name: string;
  guest: boolean;
  startTime: number;
  open: boolean;
  /** Set only once closed (a tombstone): ms epoch, used to prune old tombstones.
   * Absent while open. */
  closedAt?: number;
  /** ms epoch this entry was locally written — by `applyOpened`/`applyClosed`
   * (the caller's `Date.now()`), or by `applySnapshot` (that snapshot's
   * `requestedAtMs`). Lets `applySnapshot` tell an entry that's genuinely
   * absent from an older snapshot apart from one written locally *after* the
   * snapshot was requested — see its doc comment. */
  receivedAt: number;
}

/** The live store: one entry per period id, open or a closed tombstone. Kept as a
 * plain `ReadonlyMap` rather than a class so every operation below is a pure
 * function of (state, input) -> new state, easy to unit test in isolation from
 * React and from Ably. */
export type LivePeriodsState = ReadonlyMap<string, LiveEntry>;

/** How long a closed tombstone is kept around to block a late, lower-version
 * `period.opened` from resurrecting it, before it's pruned. */
export const TOMBSTONE_TTL_MS = 24 * 60 * 60 * 1000;

export function createLivePeriodsState(): LivePeriodsState {
  return new Map();
}

/** Applies a `period.opened` message/result. A no-op unless this is a period we've
 * never seen, or a version strictly newer than what we have — so a duplicate
 * delivery (Ably's at-least-once guarantee) or a message that arrives out of order
 * behind a newer one changes nothing. `nowMs` is stamped as the entry's
 * `receivedAt`, so a later `applySnapshot` that predates it doesn't drop it. */
export function applyOpened(
  state: LivePeriodsState,
  payload: PeriodOpenedPayload,
  nowMs: number,
): LivePeriodsState {
  const existing = state.get(payload.periodId);
  if (existing && existing.version >= payload.version) {
    return state;
  }
  const next = new Map(state);
  next.set(payload.periodId, {
    version: payload.version,
    name: payload.name,
    guest: payload.guest,
    startTime: payload.startTime,
    open: true,
    receivedAt: nowMs,
  });
  return next;
}

/** Applies a `period.closed` message/result, same version rule as `applyOpened`.
 * The closed entry is kept as a tombstone (`open: false`), not deleted, so a
 * `period.opened` that arrives late with a lower version can't bring it back —
 * see `applySnapshot` and `pruneTombstones`. */
export function applyClosed(
  state: LivePeriodsState,
  payload: PeriodClosedPayload,
  nowMs: number,
): LivePeriodsState {
  const existing = state.get(payload.periodId);
  if (existing && existing.version >= payload.version) {
    return state;
  }
  const next = new Map(state);
  next.set(payload.periodId, {
    version: payload.version,
    name: payload.name,
    guest: payload.guest,
    startTime: payload.startTime,
    open: false,
    closedAt: nowMs,
    receivedAt: nowMs,
  });
  return next;
}

/**
 * Replaces the open set with a fresh `periods(onlyActive: true)` snapshot.
 * `requestedAtMs` is when the caller *started* that snapshot fetch (not when it
 * resolved) — see below for why that distinction matters.
 *
 * For each row in the snapshot, an existing entry (open or tombstone) wins over
 * it only if the existing entry's version is already at least as new — otherwise
 * the snapshot's row is taken as-is, stamped with `requestedAtMs` as its own
 * `receivedAt` so a later, actually-newer snapshot can still drop it normally.
 * Existing tombstones the snapshot doesn't mention are carried over (the
 * snapshot only lists *open* periods, so it never says a tombstone is wrong),
 * which is what stops a stale `period.opened` replay from resurrecting
 * something already confirmed closed.
 *
 * An entry this client currently has as *open* but that's missing from the
 * snapshot is dropped — unless its `receivedAt` is after `requestedAtMs`, i.e.
 * it was written locally (by `applyOwnResult`, or by a live message applied
 * outside a resync's buffering) after the snapshot was requested, so it's
 * genuinely still open and just not on this older snapshot. A message buffered
 * *during* an in-flight resync fetch isn't affected either way: the caller
 * (`LivePeriodsProvider`) replays those straight after calling this, which
 * reapplies any such `period.opened` and restores it.
 */
export function applySnapshot(
  state: LivePeriodsState,
  snapshot: ReadonlyArray<SnapshotEntry>,
  requestedAtMs: number,
): LivePeriodsState {
  const next = new Map<string, LiveEntry>();
  for (const entry of snapshot) {
    const existing = state.get(entry.id);
    if (existing && existing.version >= entry.version) {
      next.set(entry.id, existing);
    } else {
      next.set(entry.id, {
        version: entry.version,
        name: entry.name,
        guest: entry.guest,
        startTime: entry.startTime,
        open: true,
        receivedAt: requestedAtMs,
      });
    }
  }
  for (const [id, entry] of state) {
    if (next.has(id)) continue;
    if (!entry.open || entry.receivedAt > requestedAtMs) {
      next.set(id, entry);
    }
  }
  return pruneTombstones(next, requestedAtMs);
}

/** Drops closed tombstones older than `TOMBSTONE_TTL_MS`, so the store doesn't
 * grow forever. Returns the same `state` reference when nothing changed. */
export function pruneTombstones(
  state: LivePeriodsState,
  nowMs: number,
): LivePeriodsState {
  let changed = false;
  const next = new Map(state);
  for (const [id, entry] of state) {
    if (
      !entry.open &&
      entry.closedAt !== undefined &&
      nowMs - entry.closedAt > TOMBSTONE_TTL_MS
    ) {
      next.delete(id);
      changed = true;
    }
  }
  return changed ? next : state;
}

function displayName(entry: Pick<LiveEntry, "name" | "guest">): string {
  return entry.guest ? `${entry.name} (Guest)` : entry.name;
}

/** Every currently-open period, oldest first — the shape `StatusCurrentDisplay`,
 * `ScanSignedInPanel` and `ScanStatusDialog` all render, guest names already
 * suffixed " (Guest)" to match what those views showed before. */
export function selectPeriods(state: LivePeriodsState): StatusPeriod[] {
  return Array.from(state.entries())
    .filter(([, entry]) => entry.open)
    .map(([id, entry]) => ({
      id,
      startTime: entry.startTime,
      name: displayName(entry),
    }))
    .sort((a, b) => a.startTime - b.startTime);
}

/** Currently signed-in guests only, oldest first, with the raw (unsuffixed) name —
 * feeds `ScanGuestDialog`'s guest list, which shows its own "(Guest)"-free name
 * next to a Sign out button. */
export function selectGuestPeriods(state: LivePeriodsState): StatusPeriod[] {
  return Array.from(state.entries())
    .filter(([, entry]) => entry.open && entry.guest)
    .map(([id, entry]) => ({
      id,
      startTime: entry.startTime,
      name: entry.name,
    }))
    .sort((a, b) => a.startTime - b.startTime);
}

/** Defensively validates an Ably message's decoded `data` against `PeriodOpenedPayload`
 * before it's trusted — the payload arrives as JSON over the network, from a server
 * we control but still worth checking the shape of rather than assuming. Returns
 * `null` for anything that doesn't match. */
export function parseOpenedPayload(data: unknown): PeriodOpenedPayload | null {
  if (typeof data !== "object" || data === null) return null;
  const d = data as Record<string, unknown>;
  if (
    typeof d.periodId === "string" &&
    typeof d.version === "number" &&
    typeof d.name === "string" &&
    typeof d.guest === "boolean" &&
    typeof d.startTime === "number"
  ) {
    return {
      periodId: d.periodId,
      version: d.version,
      name: d.name,
      guest: d.guest,
      startTime: d.startTime,
    };
  }
  return null;
}

/** Same defensive validation as `parseOpenedPayload`, for `period.closed`. */
export function parseClosedPayload(data: unknown): PeriodClosedPayload | null {
  if (typeof data !== "object" || data === null) return null;
  const d = data as Record<string, unknown>;
  if (
    typeof d.periodId === "string" &&
    typeof d.version === "number" &&
    typeof d.name === "string" &&
    typeof d.guest === "boolean" &&
    typeof d.startTime === "number" &&
    (d.endTime === null || typeof d.endTime === "number") &&
    typeof d.deleted === "boolean"
  ) {
    return {
      periodId: d.periodId,
      version: d.version,
      name: d.name,
      guest: d.guest,
      startTime: d.startTime,
      endTime: d.endTime,
      deleted: d.deleted,
    };
  }
  return null;
}

import { describe, it, expect } from "vitest";
import {
  applyClosed,
  applyOpened,
  applySnapshot,
  createLivePeriodsState,
  parseClosedPayload,
  parseOpenedPayload,
  pruneTombstones,
  selectGuestPeriods,
  selectPeriods,
  TOMBSTONE_TTL_MS,
  type PeriodClosedPayload,
  type PeriodOpenedPayload,
  type SnapshotEntry,
} from "./livePeriods";

const NOW = 1_700_000_000_000; // arbitrary fixed ms epoch

function opened(
  overrides: Partial<PeriodOpenedPayload> = {},
): PeriodOpenedPayload {
  return {
    periodId: "period-1",
    version: 1,
    name: "Alice Anderson",
    guest: false,
    startTime: 1_700_000_000,
    ...overrides,
  };
}

function closed(
  overrides: Partial<PeriodClosedPayload> = {},
): PeriodClosedPayload {
  return {
    periodId: "period-1",
    version: 2,
    name: "Alice Anderson",
    guest: false,
    startTime: 1_700_000_000,
    endTime: 1_700_003_600,
    deleted: false,
    ...overrides,
  };
}

describe("applyOpened / selectPeriods", () => {
  it("adds a newly opened period", () => {
    const state = applyOpened(createLivePeriodsState(), opened(), NOW);
    expect(selectPeriods(state)).toEqual([
      { id: "period-1", startTime: 1_700_000_000, name: "Alice Anderson" },
    ]);
  });

  it("suffixes a guest's name with '(Guest)'", () => {
    const state = applyOpened(
      createLivePeriodsState(),
      opened({ periodId: "period-2", name: "Jamie Visitor", guest: true }),
      NOW,
    );
    expect(selectPeriods(state)[0].name).toBe("Jamie Visitor (Guest)");
  });

  it("orders periods oldest sign-in first", () => {
    let state = createLivePeriodsState();
    state = applyOpened(
      state,
      opened({ periodId: "later", name: "Later", startTime: 200 }),
      NOW,
    );
    state = applyOpened(
      state,
      opened({ periodId: "earlier", name: "Earlier", startTime: 100 }),
      NOW,
    );
    expect(selectPeriods(state).map((p) => p.id)).toEqual(["earlier", "later"]);
  });

  it("ignores a duplicate delivery of the same version", () => {
    let state = applyOpened(
      createLivePeriodsState(),
      opened({ version: 1 }),
      NOW,
    );
    // Ably's at-least-once delivery can redeliver the same message.
    state = applyOpened(
      state,
      opened({ version: 1, name: "Renamed Somehow" }),
      NOW,
    );
    expect(selectPeriods(state)[0].name).toBe("Alice Anderson");
  });

  it("ignores an out-of-order opened with a lower version than what's known", () => {
    let state = applyOpened(
      createLivePeriodsState(),
      opened({ version: 5 }),
      NOW,
    );
    state = applyOpened(state, opened({ version: 3, name: "Stale" }), NOW);
    expect(selectPeriods(state)[0].name).toBe("Alice Anderson");
  });

  it("applies a strictly newer version, replacing prior fields", () => {
    let state = applyOpened(
      createLivePeriodsState(),
      opened({ version: 1 }),
      NOW,
    );
    state = applyOpened(
      state,
      opened({ version: 2, startTime: 1_700_000_500 }),
      NOW,
    );
    expect(selectPeriods(state)[0].startTime).toBe(1_700_000_500);
  });
});

describe("applyClosed", () => {
  it("removes a closed period from the open selectors", () => {
    let state = applyOpened(
      createLivePeriodsState(),
      opened({ version: 1 }),
      NOW,
    );
    state = applyClosed(state, closed({ version: 2 }), NOW);
    expect(selectPeriods(state)).toEqual([]);
  });

  it("ignores a closed message with a version no newer than what's known", () => {
    let state = applyOpened(
      createLivePeriodsState(),
      opened({ version: 5 }),
      NOW,
    );
    state = applyClosed(state, closed({ version: 3 }), NOW);
    // The stale close must not have won: the period is still open.
    expect(selectPeriods(state)).toHaveLength(1);
  });

  it("handles a close arriving for an id never seen open (still tombstones it)", () => {
    const state = applyClosed(
      createLivePeriodsState(),
      closed({ periodId: "never-opened", version: 1 }),
      NOW,
    );
    expect(selectPeriods(state)).toEqual([]);
  });

  it("out-of-order: a closed message that arrives before its opened counterpart wins", () => {
    // Network reordering: the close (higher version) lands first.
    let state = applyClosed(
      createLivePeriodsState(),
      closed({ version: 2 }),
      NOW,
    );
    // The opened event for the same write (lower version) arrives after.
    state = applyOpened(state, opened({ version: 1 }), NOW);
    // Must not resurrect: the tombstone at version 2 still wins.
    expect(selectPeriods(state)).toEqual([]);
  });
});

describe("applySnapshot", () => {
  function snapshotEntry(
    overrides: Partial<SnapshotEntry> = {},
  ): SnapshotEntry {
    return {
      id: "period-1",
      version: 1,
      name: "Alice Anderson",
      guest: false,
      startTime: 1_700_000_000,
      ...overrides,
    };
  }

  it("populates an empty store from the snapshot", () => {
    const state = applySnapshot(
      createLivePeriodsState(),
      [snapshotEntry()],
      NOW,
    );
    expect(selectPeriods(state)).toHaveLength(1);
  });

  it("keeps a newer locally-known version instead of an older snapshot row", () => {
    // e.g. our own optimistic apply, or a live event, arrived after the
    // snapshot request went out but the snapshot itself still reflects the
    // pre-write state.
    let state = applyOpened(
      createLivePeriodsState(),
      opened({ version: 5, name: "Fresher" }),
      NOW - 1000,
    );
    state = applySnapshot(state, [snapshotEntry({ version: 1 })], NOW);
    expect(selectPeriods(state)[0].name).toBe("Fresher");
  });

  it("takes the snapshot's row when it's newer than what's known", () => {
    let state = applyOpened(
      createLivePeriodsState(),
      opened({ version: 1, name: "Stale" }),
      NOW - 1000,
    );
    state = applySnapshot(
      state,
      [snapshotEntry({ version: 2, name: "Fresh From Snapshot" })],
      NOW,
    );
    expect(selectPeriods(state)[0].name).toBe("Fresh From Snapshot");
  });

  it("carries over a tombstone the snapshot doesn't mention, blocking resurrection", () => {
    let state = applyOpened(
      createLivePeriodsState(),
      opened({ version: 1 }),
      NOW - 1000,
    );
    state = applyClosed(state, closed({ version: 2 }), NOW);
    // The snapshot (an onlyActive query) naturally has no row for a closed period.
    state = applySnapshot(state, [], NOW);
    // A late opened at version 1 must still not resurrect it.
    state = applyOpened(state, opened({ version: 1 }), NOW);
    expect(selectPeriods(state)).toEqual([]);
  });

  it("drops an open period that predates the snapshot request and is missing from it", () => {
    // Genuinely gone: this entry was written well before the snapshot fetch
    // even started, so its absence from the response is trustworthy.
    let state = applyOpened(
      createLivePeriodsState(),
      opened({ version: 1 }),
      NOW - 1000,
    );
    state = applySnapshot(state, [], NOW);
    expect(selectPeriods(state)).toEqual([]);
  });

  it("keeps an open period written after the snapshot request started even though it's missing from the snapshot", () => {
    // e.g. applyOwnResult from the kiosk's own scan mutation, or a live
    // message applied outside a resync's buffering, landing while this
    // snapshot's fetch was already in flight — genuinely still open, just not
    // on this older snapshot (see applySnapshot's doc comment).
    let state = applyOpened(
      createLivePeriodsState(),
      opened({ version: 1 }),
      NOW + 1000,
    );
    state = applySnapshot(state, [], NOW);
    expect(selectPeriods(state)).toHaveLength(1);
  });

  it("prunes expired tombstones as part of applying a snapshot", () => {
    let state = applyOpened(
      createLivePeriodsState(),
      opened({ version: 1 }),
      NOW - TOMBSTONE_TTL_MS - 1,
    );
    const closedAt = NOW - TOMBSTONE_TTL_MS - 1;
    state = applyClosed(state, closed({ version: 2 }), closedAt);
    const before = applySnapshot(state, [], closedAt);
    // Not yet expired relative to its own close time.
    expect(before.has("period-1")).toBe(true);
    const after = applySnapshot(state, [], NOW);
    expect(after.has("period-1")).toBe(false);
  });
});

describe("pruneTombstones", () => {
  it("removes a tombstone older than the TTL", () => {
    let state = applyOpened(
      createLivePeriodsState(),
      opened({ version: 1 }),
      NOW - TOMBSTONE_TTL_MS - 1,
    );
    state = applyClosed(
      state,
      closed({ version: 2 }),
      NOW - TOMBSTONE_TTL_MS - 1,
    );
    const pruned = pruneTombstones(state, NOW);
    expect(pruned.has("period-1")).toBe(false);
  });

  it("keeps a tombstone within the TTL", () => {
    let state = applyOpened(
      createLivePeriodsState(),
      opened({ version: 1 }),
      NOW - 1000,
    );
    state = applyClosed(state, closed({ version: 2 }), NOW - 1000);
    const pruned = pruneTombstones(state, NOW);
    expect(pruned.has("period-1")).toBe(true);
  });

  it("leaves an open entry alone regardless of age", () => {
    const state = applyOpened(
      createLivePeriodsState(),
      opened({ version: 1 }),
      NOW,
    );
    const pruned = pruneTombstones(state, NOW + TOMBSTONE_TTL_MS * 10);
    expect(pruned.has("period-1")).toBe(true);
  });

  it("returns the same reference when nothing changed", () => {
    const state = applyOpened(
      createLivePeriodsState(),
      opened({ version: 1 }),
      NOW,
    );
    expect(pruneTombstones(state, NOW)).toBe(state);
  });
});

describe("selectGuestPeriods", () => {
  it("includes only guests, with the raw (unsuffixed) name", () => {
    let state = createLivePeriodsState();
    state = applyOpened(
      state,
      opened({ periodId: "member", name: "Alice Anderson", guest: false }),
      NOW,
    );
    state = applyOpened(
      state,
      opened({
        periodId: "guest",
        name: "Jamie Visitor",
        guest: true,
        startTime: 50,
      }),
      NOW,
    );
    expect(selectGuestPeriods(state)).toEqual([
      { id: "guest", startTime: 50, name: "Jamie Visitor" },
    ]);
  });
});

describe("parseOpenedPayload", () => {
  it("accepts a well-formed payload", () => {
    expect(parseOpenedPayload(opened())).toEqual(opened());
  });

  it.each([
    null,
    undefined,
    "not an object",
    42,
    {},
    { periodId: "x" },
    { ...opened(), version: "1" },
    { ...opened(), guest: "false" },
    { ...opened(), startTime: "100" },
  ])("rejects malformed data: %j", (data) => {
    expect(parseOpenedPayload(data)).toBeNull();
  });
});

describe("parseClosedPayload", () => {
  it("accepts a well-formed payload", () => {
    expect(parseClosedPayload(closed())).toEqual(closed());
  });

  it("accepts a null endTime (an open period deleted without ever closing)", () => {
    const payload = closed({ endTime: null, deleted: true });
    expect(parseClosedPayload(payload)).toEqual(payload);
  });

  it.each([
    null,
    "nope",
    { ...closed(), deleted: "false" },
    { ...closed(), endTime: "1700003600" },
  ])("rejects malformed data: %j", (data) => {
    expect(parseClosedPayload(data)).toBeNull();
  });
});

import { act, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  RealtimeChannelStateChange,
  RealtimeConnection,
  RealtimeConnectionStateChange,
  RealtimeMessage,
} from "../lib/realtimeClient";
import { KioskEnvironmentContext } from "./KioskEnvironmentContext";
import { KioskRealtimeContext } from "./KioskRealtimeContext";
import { KioskSessionContext, type KioskSession } from "./KioskSessionContext";
import { SessionPushListener } from "./SessionPushListener";

vi.mock("../lib/enrollmentKey", () => ({ fetchKeySessionId: vi.fn() }));
vi.mock("../lib/kioskKey", () => ({
  getOrCreateKioskKey: vi.fn(async () => ({ fake: "key" })),
}));
import { fetchKeySessionId } from "../lib/enrollmentKey";

const session: KioskSession = {
  id: "s1",
  name: "Front desk",
  config: {},
  version: 3,
  location: { id: "loc1", name: "Unit A" },
};

function makeFakeConnection() {
  let messageListeners: Array<(m: RealtimeMessage) => void> = [];
  let stateListeners: Array<(c: RealtimeChannelStateChange) => void> = [];
  const connectionListeners = new Set<
    (c: RealtimeConnectionStateChange) => void
  >();
  const channelNames: string[] = [];
  let released = 0;
  const connection: RealtimeConnection = {
    state: "connected",
    channel: (name) => {
      channelNames.push(name);
      return {
        subscribe: (l) => messageListeners.push(l),
        onChannelStateChange: (l) => stateListeners.push(l),
        release: () => {
          released += 1;
          messageListeners = [];
          stateListeners = [];
        },
      };
    },
    onConnectionStateChange: (listener) => {
      connectionListeners.add(listener);
      return () => connectionListeners.delete(listener);
    },
    close: () => {},
  };
  return {
    connection,
    channelNames,
    releasedCount: () => released,
    emit: (message: RealtimeMessage) =>
      act(() => messageListeners.forEach((l) => l(message))),
    emitState: (change: RealtimeChannelStateChange) =>
      act(() => stateListeners.forEach((l) => l(change))),
    emitConnectionState: (change: RealtimeConnectionStateChange) =>
      act(() => connectionListeners.forEach((l) => l(change))),
    connectionListenerCount: () => connectionListeners.size,
  };
}

function renderListener() {
  const fake = makeFakeConnection();
  const onUpdated = vi.fn();
  const refreshNow = vi.fn();
  const onLiveChange = vi.fn();
  const view = render(
    <KioskEnvironmentContext.Provider
      value={{
        profile: "default",
        onKeyEnrolled: vi.fn(),
      }}
    >
      <KioskSessionContext.Provider value={{ session }}>
        <KioskRealtimeContext.Provider
          value={{
            status: "ready",
            connection: fake.connection,
            locationChannel: "kiosk:test:loc1",
            sessionChannel: "kiosk:test:session:s1",
            retry: vi.fn(),
          }}
        >
          <SessionPushListener
            onUpdated={onUpdated}
            refreshNow={refreshNow}
            onLiveChange={onLiveChange}
          />
        </KioskRealtimeContext.Provider>
      </KioskSessionContext.Provider>
    </KioskEnvironmentContext.Provider>,
  );
  return { ...fake, onUpdated, refreshNow, onLiveChange, ...view };
}

afterEach(() => {
  vi.useRealTimers();
  vi.mocked(fetchKeySessionId).mockReset();
});

describe("SessionPushListener", () => {
  it("subscribes to the session channel, and releases it on unmount", () => {
    const { channelNames, releasedCount, unmount } = renderListener();
    expect(channelNames).toEqual(["kiosk:test:session:s1"]);
    unmount();
    expect(releasedCount()).toBe(1);
  });

  it("hands a session.updated push on, and drops a malformed one", () => {
    const { emit, onUpdated } = renderListener();
    emit({
      name: "session.updated",
      data: { sessionId: "s1", version: 4, name: "Back", config: { a: 1 } },
    });
    emit({ name: "session.updated", data: { sessionId: "s1" } });
    expect(onUpdated).toHaveBeenCalledExactlyOnceWith({
      sessionId: "s1",
      version: 4,
      name: "Back",
      config: { a: 1 },
    });
  });

  it("refetches on a newer session.changed, and ignores a stale one", async () => {
    vi.mocked(fetchKeySessionId).mockResolvedValue("s-new");
    const { emit, refreshNow } = renderListener();
    emit({ name: "session.changed", data: { sessionId: "s1", version: 3 } });
    emit({ name: "session.changed", data: { sessionId: "s2", version: 9 } });
    expect(refreshNow).not.toHaveBeenCalled();

    emit({ name: "session.changed", data: { sessionId: "s1", version: 4 } });
    await vi.waitFor(() => expect(refreshNow).toHaveBeenCalledOnce());
  });

  it("refetches on a non-resumed attach only", () => {
    const { emitState, refreshNow } = renderListener();
    emitState({ current: "attached", resumed: true });
    expect(refreshNow).not.toHaveBeenCalled();
    emitState({ current: "attached", resumed: false });
    expect(refreshNow).toHaveBeenCalledOnce();
  });

  it("reports live only while the channel is attached and the connection is up", () => {
    const {
      emitState,
      emitConnectionState,
      onLiveChange,
      connectionListenerCount,
      unmount,
    } = renderListener();

    emitState({ current: "attached", resumed: false });
    expect(onLiveChange).toHaveBeenLastCalledWith(true);

    // Disconnected: the channel can still read "attached" for a while.
    emitConnectionState({ current: "disconnected" });
    expect(onLiveChange).toHaveBeenLastCalledWith(false);
    emitConnectionState({ current: "connected" });
    expect(onLiveChange).toHaveBeenLastCalledWith(true);

    emitState({ current: "suspended", resumed: false });
    expect(onLiveChange).toHaveBeenLastCalledWith(false);
    emitState({ current: "attached", resumed: true });
    expect(onLiveChange).toHaveBeenLastCalledWith(true);

    unmount();
    expect(onLiveChange).toHaveBeenLastCalledWith(false);
    expect(connectionListenerCount()).toBe(0);
  });

  it("waits for the key to resolve to the new session before refetching", async () => {
    vi.useFakeTimers();
    vi.mocked(fetchKeySessionId)
      .mockResolvedValueOnce(null) // the index hasn't caught up yet
      .mockResolvedValueOnce("s1") // still the old session
      .mockResolvedValueOnce("s-new");
    const { emit, refreshNow } = renderListener();

    emit({ name: "session.changed", data: { sessionId: "s1", version: 4 } });
    await vi.advanceTimersByTimeAsync(0);
    expect(fetchKeySessionId).toHaveBeenCalledTimes(1);
    expect(refreshNow).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(500);
    expect(fetchKeySessionId).toHaveBeenCalledTimes(2);
    expect(refreshNow).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1000);
    expect(fetchKeySessionId).toHaveBeenCalledTimes(3);
    expect(refreshNow).toHaveBeenCalledOnce();
  });

  it("refetches anyway once the retries run out (the session was deleted)", async () => {
    vi.useFakeTimers();
    vi.mocked(fetchKeySessionId).mockResolvedValue(null);
    const { emit, refreshNow } = renderListener();

    emit({ name: "session.changed", data: { sessionId: "s1", version: 4 } });
    await vi.advanceTimersByTimeAsync(500 + 1000 + 2000 + 4000 + 8000);
    expect(fetchKeySessionId).toHaveBeenCalledTimes(6);
    expect(refreshNow).toHaveBeenCalledOnce();
  });
});

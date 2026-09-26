import { act, render, screen, waitFor } from "@testing-library/react";
import UserEvent from "@testing-library/user-event";
import {
  afterAll,
  afterEach,
  beforeAll,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import { setupServer } from "msw/node";
import { graphql, HttpResponse, type GraphQLResponseResolver } from "msw";
import { RelayEnvironmentProvider } from "react-relay";
import { getGraphQLEndpoint } from "../../lib/api";
import { createKioskGraphQLEnvironment } from "../../lib/environments";
import { KioskSessionContext } from "./KioskSessionContext";
import { LivePeriodsProvider } from "./LivePeriodsProvider";
import { useLivePeriods } from "./useLivePeriods";
import { EVENT_PERIOD_OPENED } from "../lib/livePeriods";
import type {
  RealtimeChannelStateChange,
  RealtimeClient,
  RealtimeConnectionStateChange,
  RealtimeMessage,
} from "../lib/realtimeClient";

// LivePeriodsProvider talks to ably-js only through this seam — replace it with
// a fake whose listeners tests can fire by hand, so no real network or jsdom
// WebSocket is ever involved.
vi.mock("../lib/realtimeClient", () => ({
  createRealtimeClient: vi.fn(),
}));
import { createRealtimeClient } from "../lib/realtimeClient";

/** A controllable stand-in for the object `createRealtimeClient` resolves to. */
function makeFakeRealtimeClient() {
  const messageListeners: Array<(m: RealtimeMessage) => void> = [];
  const channelListeners: Array<(c: RealtimeChannelStateChange) => void> = [];
  const connectionListeners: Array<(c: RealtimeConnectionStateChange) => void> =
    [];
  let closed = false;

  const client: RealtimeClient = {
    subscribe: (listener) => messageListeners.push(listener),
    onChannelStateChange: (listener) => channelListeners.push(listener),
    onConnectionStateChange: (listener) => connectionListeners.push(listener),
    close: () => {
      closed = true;
    },
  };

  return {
    client,
    emitMessage: (message: RealtimeMessage) =>
      messageListeners.forEach((l) => l(message)),
    emitChannelState: (change: RealtimeChannelStateChange) =>
      channelListeners.forEach((l) => l(change)),
    emitConnectionState: (change: RealtimeConnectionStateChange) =>
      connectionListeners.forEach((l) => l(change)),
    isClosed: () => closed,
  };
}

const relayUrl = getGraphQLEndpoint();
const relayEndpoint = graphql.link(relayUrl);

const TOKEN_RESPONSE = {
  data: {
    kioskRealtimeToken: {
      channel: "kiosk:seslogin_test:loc1",
      tokenRequest: {
        keyName: "app.key",
        ttl: 3_600_000,
        capability: '{"kiosk:seslogin_test:loc1":["subscribe"]}',
        clientId: "session:sess1",
        timestamp: 1_700_000_000_000,
        nonce: "0123456789abcdef",
        mac: "deadbeef",
      },
    },
  },
};

const NULL_TOKEN_RESPONSE = { data: { kioskRealtimeToken: null } };

function tokenHandler(
  response: typeof TOKEN_RESPONSE | typeof NULL_TOKEN_RESPONSE = TOKEN_RESPONSE,
) {
  return relayEndpoint.query("KioskRealtimeTokenQuery", () =>
    HttpResponse.json(response),
  );
}

type SnapshotNode = {
  id: string;
  version: number;
  startTime: number;
  guestName: string | null;
  person: { id: string; firstName: string; lastName: string } | null;
};

function snapshotResponseBody(nodes: SnapshotNode[]) {
  return {
    data: {
      session: {
        location: { periods: { edges: nodes.map((node) => ({ node })) } },
      },
    },
  };
}

function snapshotHandler(nodes: SnapshotNode[]) {
  return relayEndpoint.query("LivePeriodsSnapshotQuery", () =>
    HttpResponse.json(snapshotResponseBody(nodes)),
  );
}

const server = setupServer(tokenHandler(), snapshotHandler([]));

beforeAll(() => server.listen());
afterEach(() => {
  server.resetHandlers();
  vi.mocked(createRealtimeClient).mockReset();
});
afterAll(() => server.close());

const session = {
  id: "sess1",
  name: "Test Kiosk",
  config: { signedInStatus: true },
  location: { id: "loc1", name: "Test Location" },
};

function Consumer() {
  const { periods, live, loading, error, retry } = useLivePeriods();
  return (
    <div>
      <div data-testid="loading">{String(loading)}</div>
      <div data-testid="live">{String(live)}</div>
      <div data-testid="error">{error ?? ""}</div>
      <ul>
        {periods.map((p) => (
          <li key={p.id}>{p.name}</li>
        ))}
      </ul>
      <button onClick={retry}>retry</button>
    </div>
  );
}

function renderProvider() {
  const environment = createKioskGraphQLEnvironment(
    () => "test-token",
    () => {},
  );
  return render(
    <RelayEnvironmentProvider environment={environment}>
      <KioskSessionContext.Provider value={{ session }}>
        <LivePeriodsProvider>
          <Consumer />
        </LivePeriodsProvider>
      </KioskSessionContext.Provider>
    </RelayEnvironmentProvider>,
  );
}

function node(overrides: Partial<SnapshotNode> = {}): SnapshotNode {
  return {
    id: "period-1",
    version: 1,
    startTime: Math.floor(Date.now() / 1000) - 60,
    guestName: null,
    person: { id: "person-1", firstName: "Alice", lastName: "Anderson" },
    ...overrides,
  };
}

describe("LivePeriodsProvider", () => {
  it("polls the snapshot when the realtime token is null", async () => {
    server.use(tokenHandler(NULL_TOKEN_RESPONSE), snapshotHandler([node()]));

    renderProvider();

    await waitFor(() =>
      expect(screen.getByText("Alice Anderson")).toBeInTheDocument(),
    );
    expect(screen.getByTestId("live")).toHaveTextContent("false");
    expect(createRealtimeClient).not.toHaveBeenCalled();
  });

  it("goes live once the channel attaches, after fetching a snapshot", async () => {
    const fake = makeFakeRealtimeClient();
    vi.mocked(createRealtimeClient).mockResolvedValue(fake.client);
    server.use(snapshotHandler([node()]));

    renderProvider();

    await waitFor(() => expect(createRealtimeClient).toHaveBeenCalledOnce());
    expect(screen.getByTestId("live")).toHaveTextContent("false");

    // Every attach — including the very first — arrives with resumed: false,
    // so it triggers a snapshot fetch before the list is trusted.
    act(() => fake.emitChannelState({ current: "attached", resumed: false }));

    await waitFor(() =>
      expect(screen.getByText("Alice Anderson")).toBeInTheDocument(),
    );
    expect(screen.getByTestId("live")).toHaveTextContent("true");
    expect(screen.getByTestId("loading")).toHaveTextContent("false");
  });

  it("applies a live message immediately, with no extra network request", async () => {
    let snapshotCalls = 0;
    const countingSnapshot: GraphQLResponseResolver = () => {
      snapshotCalls += 1;
      return HttpResponse.json(snapshotResponseBody([]));
    };
    server.use(
      relayEndpoint.query("LivePeriodsSnapshotQuery", countingSnapshot),
    );

    const fake = makeFakeRealtimeClient();
    vi.mocked(createRealtimeClient).mockResolvedValue(fake.client);

    renderProvider();

    await waitFor(() => expect(createRealtimeClient).toHaveBeenCalledOnce());
    act(() => fake.emitChannelState({ current: "attached", resumed: false }));
    await waitFor(() => expect(snapshotCalls).toBe(1));

    act(() =>
      fake.emitMessage({
        name: EVENT_PERIOD_OPENED,
        data: {
          periodId: "period-2",
          version: 1,
          name: "Bob Brown",
          guest: false,
          startTime: Math.floor(Date.now() / 1000),
        },
      }),
    );

    await waitFor(() =>
      expect(screen.getByText("Bob Brown")).toBeInTheDocument(),
    );
    // The message updated the list on its own — no second snapshot fetch.
    expect(snapshotCalls).toBe(1);
  });

  it("resumed attach doesn't need a resync and keeps existing data", async () => {
    const fake = makeFakeRealtimeClient();
    vi.mocked(createRealtimeClient).mockResolvedValue(fake.client);
    let snapshotCalls = 0;
    server.use(
      relayEndpoint.query("LivePeriodsSnapshotQuery", () => {
        snapshotCalls += 1;
        return HttpResponse.json(snapshotResponseBody([node()]));
      }),
    );

    renderProvider();
    await waitFor(() => expect(createRealtimeClient).toHaveBeenCalledOnce());

    act(() => fake.emitChannelState({ current: "attached", resumed: false }));
    await waitFor(() => expect(snapshotCalls).toBe(1));

    // A brief reconnect Ably itself resumed (continuity preserved) needs no
    // fresh snapshot — unlike a `resumed: false` reattach, which does (see the
    // "goes live" test above). Note this deliberately skips an intermediate
    // "detached" state: going through one would itself start the polling
    // fallback (see the buffering test's channel-fallback coverage) and add
    // an extra snapshot call unrelated to what this test checks.
    act(() => fake.emitChannelState({ current: "attached", resumed: true }));

    await waitFor(() =>
      expect(screen.getByTestId("live")).toHaveTextContent("true"),
    );
    expect(snapshotCalls).toBe(1);
  });

  it("buffers messages received while a resync snapshot is in flight, then replays them", async () => {
    const fake = makeFakeRealtimeClient();
    vi.mocked(createRealtimeClient).mockResolvedValue(fake.client);

    let releaseSnapshot: (() => void) | undefined;
    const gate = new Promise<void>((resolve) => {
      releaseSnapshot = resolve;
    });
    server.use(
      relayEndpoint.query("LivePeriodsSnapshotQuery", async () => {
        await gate;
        // The snapshot itself has nothing — the period below only exists
        // because of the buffered `period.opened` replayed after it.
        return HttpResponse.json(snapshotResponseBody([]));
      }),
    );

    renderProvider();
    await waitFor(() => expect(createRealtimeClient).toHaveBeenCalledOnce());

    act(() => fake.emitChannelState({ current: "attached", resumed: false }));
    // The resync's snapshot fetch is now in flight (gated on `gate`).

    act(() =>
      fake.emitMessage({
        name: EVENT_PERIOD_OPENED,
        data: {
          periodId: "period-3",
          version: 1,
          name: "Carol Clark",
          guest: false,
          startTime: Math.floor(Date.now() / 1000),
        },
      }),
    );

    // Still buffered: the snapshot hasn't resolved (and reported empty) yet.
    expect(screen.queryByText("Carol Clark")).not.toBeInTheDocument();

    releaseSnapshot!();

    await waitFor(() =>
      expect(screen.getByText("Carol Clark")).toBeInTheDocument(),
    );
  });

  it("overlapping resyncs don't lose a message buffered before the second one starts", async () => {
    const fake = makeFakeRealtimeClient();
    vi.mocked(createRealtimeClient).mockResolvedValue(fake.client);

    let releaseSnapshot: (() => void) | undefined;
    const gate = new Promise<void>((resolve) => {
      releaseSnapshot = resolve;
    });
    server.use(
      relayEndpoint.query("LivePeriodsSnapshotQuery", async () => {
        await gate;
        return HttpResponse.json(snapshotResponseBody([]));
      }),
    );

    renderProvider();
    await waitFor(() => expect(createRealtimeClient).toHaveBeenCalledOnce());

    // First resync starts; its snapshot fetch is gated in flight.
    act(() => fake.emitChannelState({ current: "attached", resumed: false }));

    // Buffered by the first resync, before any second one exists.
    act(() =>
      fake.emitMessage({
        name: EVENT_PERIOD_OPENED,
        data: {
          periodId: "period-4",
          version: 1,
          name: "Dana Diaz",
          guest: false,
          startTime: Math.floor(Date.now() / 1000),
        },
      }),
    );

    // A second non-resumed reattach starts an overlapping resync before the
    // first one's fetch has resolved. The buggy version reset the shared
    // queue here (`messageQueue = []`), silently discarding the message the
    // first resync had already buffered.
    act(() => fake.emitChannelState({ current: "attached", resumed: false }));

    // Still buffered: neither resync's snapshot fetch has resolved yet (both
    // share the same gate).
    expect(screen.queryByText("Dana Diaz")).not.toBeInTheDocument();

    releaseSnapshot!();

    await waitFor(() =>
      expect(screen.getByText("Dana Diaz")).toBeInTheDocument(),
    );
  });

  it("falls back to polling and shows a retryable error when the snapshot fails", async () => {
    server.use(
      tokenHandler(NULL_TOKEN_RESPONSE),
      relayEndpoint.query("LivePeriodsSnapshotQuery", () =>
        HttpResponse.json({ errors: [{ message: "boom" }] }, { status: 500 }),
      ),
    );

    renderProvider();

    await waitFor(() =>
      expect(screen.getByTestId("error")).not.toHaveTextContent(""),
    );

    server.use(snapshotHandler([node()]));
    const user = UserEvent.setup();
    await user.click(screen.getByRole("button", { name: "retry" }));

    await waitFor(() =>
      expect(screen.getByText("Alice Anderson")).toBeInTheDocument(),
    );
    expect(screen.getByTestId("error")).toHaveTextContent("");
  });
});

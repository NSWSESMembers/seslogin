import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { IEnvironment } from "relay-runtime";

vi.mock("relay-runtime", async (importOriginal) => ({
  ...(await importOriginal<typeof import("relay-runtime")>()),
  fetchQuery: vi.fn(),
}));
import { fetchQuery } from "relay-runtime";
import startKioskSessionFetcher, {
  LIVE_SESSION_REFRESH_INTERVAL_MS,
  SESSION_REFRESH_INTERVAL_MS,
} from "./KioskSessionFetcher";
import {
  CLOCK_SKEW_RETRY_MS,
  clearClockSkew,
  reportClockSkew,
} from "../lib/clockSkew";

const response = {
  session: {
    id: "s1",
    name: "Front desk",
    config: {},
    version: 1,
    keyExpiresAt: null,
    location: { id: "loc1", name: "Unit A" },
  },
  environment: null,
};

/** Each fetch stays pending until `resolveNext` is called. */
let pending: Array<() => void> = [];
function resolveNext() {
  pending.shift()?.();
}

beforeEach(() => {
  vi.useFakeTimers();
  // No splay: every delay is exactly the interval.
  vi.spyOn(Math, "random").mockReturnValue(0.5);
  pending = [];
  vi.mocked(fetchQuery).mockImplementation(
    () =>
      ({
        toPromise: () =>
          new Promise((resolve) => pending.push(() => resolve(response))),
      }) as unknown as ReturnType<typeof fetchQuery>,
  );
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.mocked(fetchQuery).mockReset();
});

function start(pushLive = false) {
  return startKioskSessionFetcher({
    environment: {} as IEnvironment,
    setSession: vi.fn(),
    pushLive,
  });
}

/** Lets the resolved fetch's `.then` run. */
async function settle() {
  await vi.advanceTimersByTimeAsync(0);
}

describe("startKioskSessionFetcher", () => {
  it("polls on the ordinary interval while nothing is pushed", async () => {
    const fetcher = start();
    expect(fetchQuery).toHaveBeenCalledTimes(1);
    resolveNext();
    await settle();

    await vi.advanceTimersByTimeAsync(SESSION_REFRESH_INTERVAL_MS - 1);
    expect(fetchQuery).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(fetchQuery).toHaveBeenCalledTimes(2);
    fetcher.stop();
  });

  it("stretches to the live interval while pushes are flowing", async () => {
    const fetcher = start(true);
    resolveNext();
    await settle();

    await vi.advanceTimersByTimeAsync(LIVE_SESSION_REFRESH_INTERVAL_MS - 1);
    expect(fetchQuery).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(fetchQuery).toHaveBeenCalledTimes(2);
    fetcher.stop();
  });

  it("brings a long wait forward when pushes stop", async () => {
    const fetcher = start(true);
    resolveNext();
    await settle();

    await vi.advanceTimersByTimeAsync(60_000);
    fetcher.setPushLive(false);
    // No later than the ordinary interval from the moment pushes stopped.
    await vi.advanceTimersByTimeAsync(SESSION_REFRESH_INTERVAL_MS);
    expect(fetchQuery).toHaveBeenCalledTimes(2);
    fetcher.stop();
  });

  it("refreshes now, and again straight after one already in flight", async () => {
    const fetcher = start();
    // The first refresh is still in flight.
    fetcher.refreshNow();
    expect(fetchQuery).toHaveBeenCalledTimes(1);
    resolveNext();
    await settle();
    expect(fetchQuery).toHaveBeenCalledTimes(2);

    resolveNext();
    await settle();
    fetcher.refreshNow();
    expect(fetchQuery).toHaveBeenCalledTimes(3);
    fetcher.stop();
  });
});

describe("startKioskSessionFetcher with a wrong clock", () => {
  afterEach(() => clearClockSkew());

  it("retries soon after a refusal for the clock, not at the next ordinary poll", async () => {
    vi.spyOn(console, "log").mockImplementation(() => {});
    vi.spyOn(console, "warn").mockImplementation(() => {});
    vi.mocked(fetchQuery).mockImplementationOnce(
      () =>
        ({
          toPromise: () => {
            reportClockSkew(Math.floor(Date.now() / 1000) + 600);
            return Promise.reject(new Error("This device's clock is wrong"));
          },
        }) as unknown as ReturnType<typeof fetchQuery>,
    );
    const fetcher = start();
    await settle();
    expect(fetchQuery).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(CLOCK_SKEW_RETRY_MS - 1);
    expect(fetchQuery).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(fetchQuery).toHaveBeenCalledTimes(2);
    fetcher.stop();
  });
});

describe("the live interval", () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.resetModules();
  });

  async function liveIntervalOn(environment: string) {
    vi.stubEnv("MODE", "production");
    vi.stubEnv("VITE_ENVIRONMENT", environment);
    vi.resetModules();
    return (await import("./KioskSessionFetcher"))
      .LIVE_SESSION_REFRESH_INTERVAL_MS;
  }

  it("is 15 minutes on prod", async () => {
    expect(await liveIntervalOn("prod")).toBe(15 * 60 * 1000);
  });

  it.each(["preprod", "qa", "test"])("is 2 minutes on %s", async (env) => {
    expect(await liveIntervalOn(env)).toBe(2 * 60 * 1000);
  });
});

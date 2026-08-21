import "@testing-library/jest-dom/vitest";
import { StrictMode } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import UserEvent from "@testing-library/user-event";
import { render, screen } from "@testing-library/react";
import { Environment, Network, RecordSource, Store } from "relay-runtime";
import { RelayEnvironmentProvider } from "react-relay";
import RelayErrorBoundary from "./RelayErrorBoundary";
import { useRelayRetryFetchKey } from "./relayRetryContext";
import { relayFieldLogger } from "../lib/relayFieldLogger";

function ThrowsAlways(): never {
  throw new Error("boom");
}

function ThrowsGenericRelayError(): never {
  throw new Error("Relay: Missing expected data at path 'x' in 'y'.");
}

function bufferMessage(message: string) {
  relayFieldLogger({
    kind: "relay_field_payload.error" as const,
    owner: "SomeQuery",
    fieldPath: "location.periodSummaryByCategory.0.category",
    error: { message, path: [], severity: "ERROR" as const },
    shouldThrow: true,
    handled: false,
  });
}

function makeEnvironment() {
  return new Environment({
    network: Network.create(() => {
      throw new Error("no network expected in this test");
    }),
    store: new Store(new RecordSource()),
  });
}

describe("RelayErrorBoundary", () => {
  beforeEach(() => {
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  it("shows 'Reload page' by default instead of a 'Try again' that can't guarantee a retry", async () => {
    const environment = makeEnvironment();
    // jsdom's window.location.reload isn't configurable enough for
    // vi.spyOn (or for redefining just that one property), so swap the
    // whole location object out for this test.
    const reloadSpy = vi.fn();
    const originalLocation = window.location;
    Object.defineProperty(window, "location", {
      configurable: true,
      value: { ...originalLocation, reload: reloadSpy },
    });

    try {
      render(
        <RelayEnvironmentProvider environment={environment}>
          <RelayErrorBoundary>
            <ThrowsAlways />
          </RelayErrorBoundary>
        </RelayEnvironmentProvider>,
      );

      expect(screen.getByText("Something went wrong")).toBeInTheDocument();
      expect(
        screen.queryByRole("button", { name: "Try again" }),
      ).not.toBeInTheDocument();

      const user = UserEvent.setup();
      await user.click(screen.getByRole("button", { name: "Reload page" }));

      expect(reloadSpy).toHaveBeenCalledOnce();
    } finally {
      Object.defineProperty(window, "location", {
        configurable: true,
        value: originalLocation,
      });
    }
  });

  it("invalidates the Relay store when 'Try again' is clicked (canRetry)", async () => {
    const environment = makeEnvironment();
    const commitUpdateSpy = vi.spyOn(environment, "commitUpdate");

    render(
      <RelayEnvironmentProvider environment={environment}>
        <RelayErrorBoundary canRetry>
          <ThrowsAlways />
        </RelayErrorBoundary>
      </RelayEnvironmentProvider>,
    );

    expect(screen.getByText("Something went wrong")).toBeInTheDocument();
    expect(commitUpdateSpy).not.toHaveBeenCalled();

    const user = UserEvent.setup();
    await user.click(screen.getByRole("button", { name: "Try again" }));

    // A plain resetErrorBoundary only clears React state, so any store data
    // read on the next render would still be whatever record caused the
    // throw. Invalidating the store on reset bumps its epoch so a *fresh*
    // QueryResource cache entry treats that data as stale rather than
    // reusing it — see the fetchKey test below for why invalidation alone
    // doesn't get a query that far.
    expect(commitUpdateSpy).toHaveBeenCalledOnce();
  });

  it("provides an incrementing fetchKey via useRelayRetryFetchKey on each 'Try again' click (canRetry)", async () => {
    const environment = makeEnvironment();
    // A plain boolean rather than a throw-once counter: React can invoke a
    // throwing component more than once per render pass while recovering
    // (it retries synchronously before surfacing to the boundary), so a
    // counter that flips after a single throw can flip before the boundary
    // ever renders its fallback. Toggling this from the test, after the
    // fallback is confirmed on screen, sidesteps that.
    let shouldThrow = true;

    function MaybeThrowsThenReportsFetchKey() {
      const fetchKey = useRelayRetryFetchKey();
      if (shouldThrow) {
        throw new Error("boom");
      }
      return <p>fetchKey:{fetchKey}</p>;
    }

    render(
      <RelayEnvironmentProvider environment={environment}>
        <RelayErrorBoundary canRetry>
          <MaybeThrowsThenReportsFetchKey />
        </RelayErrorBoundary>
      </RelayEnvironmentProvider>,
    );

    expect(screen.getByText("Something went wrong")).toBeInTheDocument();

    // Simulate whatever caused the error now being fixed (e.g. a corrected
    // input), so we can observe the fetchKey a real retry would see.
    shouldThrow = false;
    const user = UserEvent.setup();
    await user.click(screen.getByRole("button", { name: "Try again" }));

    // This is the actual fix for a query that already threw: useLazyLoadQuery
    // caches the thrown error per (query, variables), independent of the
    // store, so invalidateStore() alone never makes it refetch. A component
    // that threads this fetchKey into its useLazyLoadQuery call gets a fresh
    // cache entry instead, which does.
    expect(screen.getByText("fetchKey:1")).toBeInTheDocument();
  });

  it("remounts and clears its error when resetKey changes", () => {
    const environment = makeEnvironment();
    const Recovered = () => <p>recovered</p>;

    const { rerender } = render(
      <RelayEnvironmentProvider environment={environment}>
        <RelayErrorBoundary resetKey="a">
          <ThrowsAlways />
        </RelayErrorBoundary>
      </RelayEnvironmentProvider>,
    );
    expect(screen.getByText("Something went wrong")).toBeInTheDocument();

    rerender(
      <RelayEnvironmentProvider environment={environment}>
        <RelayErrorBoundary resetKey="b">
          <Recovered />
        </RelayErrorBoundary>
      </RelayEnvironmentProvider>,
    );
    expect(screen.getByText("recovered")).toBeInTheDocument();
  });

  it("recovers the real server message under StrictMode", () => {
    // The bug this guards against: recovering the buffered message via a
    // useState lazy initializer looks safe (it "only runs once" per commit)
    // but isn't — StrictMode deliberately double-invokes exactly that kind
    // of render-phase function to catch impurity like this. Since the
    // recovery drains the buffer as a side effect, the first (thrown-away)
    // invocation would consume the real message and the second would find
    // nothing. Resolving it via onError (componentDidCatch) instead is safe
    // because React guarantees that lifecycle method runs exactly once per
    // catch, StrictMode included.
    bufferMessage("Category with ID abc123 missing");
    const environment = makeEnvironment();

    render(
      <StrictMode>
        <RelayEnvironmentProvider environment={environment}>
          <RelayErrorBoundary showDetailsByDefault>
            <ThrowsGenericRelayError />
          </RelayErrorBoundary>
        </RelayEnvironmentProvider>
      </StrictMode>,
    );

    expect(
      screen.getByText("Category with ID abc123 missing"),
    ).toBeInTheDocument();
  });
});

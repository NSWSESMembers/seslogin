import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi, beforeEach } from "vitest";
import UserEvent from "@testing-library/user-event";
import { render, screen } from "@testing-library/react";
import { Environment, Network, RecordSource, Store } from "relay-runtime";
import { RelayEnvironmentProvider } from "react-relay";
import RelayErrorBoundary from "./RelayErrorBoundary";
import { useRelayRetryFetchKey } from "./relayRetryContext";

function ThrowsAlways(): never {
  throw new Error("boom");
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

  it("invalidates the Relay store when 'Try again' is clicked", async () => {
    const environment = makeEnvironment();
    const commitUpdateSpy = vi.spyOn(environment, "commitUpdate");

    render(
      <RelayEnvironmentProvider environment={environment}>
        <RelayErrorBoundary>
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

  it("provides an incrementing fetchKey via useRelayRetryFetchKey on each 'Try again' click", async () => {
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
        <RelayErrorBoundary>
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
});

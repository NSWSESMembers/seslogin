import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi, beforeEach } from "vitest";
import UserEvent from "@testing-library/user-event";
import { render, screen } from "@testing-library/react";
import { Environment, Network, RecordSource, Store } from "relay-runtime";
import { RelayEnvironmentProvider } from "react-relay";
import RelayErrorBoundary from "./RelayErrorBoundary";

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

    // This is the fix: a plain resetErrorBoundary only clears React state, so
    // useLazyLoadQuery's default store-or-network policy would re-read the same
    // poisoned record and throw again. Invalidating the store on reset bumps its
    // epoch so every mounted query treats its cached data as stale and refetches
    // on the next read.
    expect(commitUpdateSpy).toHaveBeenCalledOnce();
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

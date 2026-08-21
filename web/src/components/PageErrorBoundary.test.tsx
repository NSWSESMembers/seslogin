import "@testing-library/jest-dom/vitest";
import { StrictMode } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import UserEvent from "@testing-library/user-event";
import { render, screen } from "@testing-library/react";
import PageErrorBoundary from "./PageErrorBoundary";
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

describe("PageErrorBoundary", () => {
  beforeEach(() => {
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  it("shows the fallback and resets on 'Try again'", async () => {
    let shouldThrow = true;
    function MaybeThrows() {
      if (shouldThrow) throw new Error("boom");
      return <p>recovered</p>;
    }

    render(
      <PageErrorBoundary>
        <MaybeThrows />
      </PageErrorBoundary>,
    );
    expect(screen.getByText("Something went wrong")).toBeInTheDocument();

    shouldThrow = false;
    const user = UserEvent.setup();
    await user.click(screen.getByRole("button", { name: "Try again" }));

    expect(screen.getByText("recovered")).toBeInTheDocument();
  });

  it("remounts and clears its error when resetKey changes", () => {
    const Recovered = () => <p>recovered</p>;

    const { rerender } = render(
      <PageErrorBoundary resetKey="a">
        <ThrowsAlways />
      </PageErrorBoundary>,
    );
    expect(screen.getByText("Something went wrong")).toBeInTheDocument();

    rerender(
      <PageErrorBoundary resetKey="b">
        <Recovered />
      </PageErrorBoundary>,
    );
    expect(screen.getByText("recovered")).toBeInTheDocument();
  });

  it("recovers the real server message under StrictMode", () => {
    bufferMessage("Category with ID abc123 missing");

    render(
      <StrictMode>
        <PageErrorBoundary showDetailsByDefault>
          <ThrowsGenericRelayError />
        </PageErrorBoundary>
      </StrictMode>,
    );

    expect(
      screen.getByText("Category with ID abc123 missing"),
    ).toBeInTheDocument();
  });
});

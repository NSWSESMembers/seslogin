import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi } from "vitest";
import { act } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { RelayEnvironmentProvider } from "react-relay";
import { MockPayloadGenerator, createMockEnvironment } from "relay-test-utils";
import ScanTitleBar from "./ScanTitleBar";

// relay-test-utils reaches for jest.fn() internally; shim it for vitest.
(globalThis as unknown as { jest: { fn: typeof vi.fn } }).jest = { fn: vi.fn };

const session = vi.hoisted(() => ({
  current: {
    name: "Front desk",
    location: { id: "loc1", name: "Test A Unit" },
    config: { gamification: true } as Record<string, unknown>,
  },
}));

vi.mock("./useKioskSession", () => ({
  useKioskSession: () => session.current,
}));

vi.mock("../../lib/environmentInfo", () => ({
  useNonProdDb: () => false,
}));

function trophy(id: string, name: string, overrides = {}) {
  return {
    id,
    name,
    tagline: `${name} tagline`,
    rule: `${name} rule.`,
    minimumLabel: "3 busy days",
    holder: null,
    valueLabel: null,
    heldSince: null,
    ...overrides,
  };
}

describe("ScanTitleBar trophies", () => {
  it("opens the unit's trophy cabinet", async () => {
    session.current.config = { gamification: true };
    const environment = createMockEnvironment();
    render(
      <RelayEnvironmentProvider environment={environment}>
        <ScanTitleBar />
      </RelayEnvironmentProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Unit Trophies" }));
    act(() => {
      environment.mock.resolveMostRecentOperation((op) =>
        MockPayloadGenerator.generate(op, {
          Location: () => ({
            id: "loc1",
            trophies: [
              trophy("kettle", "The Kettle", {
                holder: { id: "p1", firstName: "Bob", lastName: "Brown" },
                valueLabel: "9 busy days",
                heldSince: 1_790_000_000,
              }),
              trophy("boomerang", "The Boomerang"),
            ],
          }),
        }),
      );
    });

    const kettle = (await screen.findByText("The Kettle")).closest("li")!;
    expect(
      screen.getByRole("heading", { name: "Test A Unit trophies" }),
    ).toBeInTheDocument();
    expect(screen.getByText(/1 of 2 are held\./)).toBeInTheDocument();
    expect(kettle).toHaveTextContent("Bob Brown");
    expect(kettle).toHaveTextContent("9 busy days · since");
    expect(kettle).toHaveTextContent(
      "The Kettle rule. Minimum 3 busy days to qualify.",
    );
    expect(kettle.querySelector("img")).toHaveAttribute(
      "src",
      "/image/badges/by-id/trophy-kettle.svg",
    );
    const boomerang = screen.getByText("The Boomerang").closest("li")!;
    expect(boomerang).toHaveTextContent("Nobody holds it yet");
  });

  it("has no Trophies button on a kiosk without badges", () => {
    session.current.config = {};
    render(
      <RelayEnvironmentProvider environment={createMockEnvironment()}>
        <ScanTitleBar />
      </RelayEnvironmentProvider>,
    );
    expect(
      screen.queryByRole("button", { name: "Unit Trophies" }),
    ).not.toBeInTheDocument();
  });
});

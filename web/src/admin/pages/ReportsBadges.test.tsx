import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi } from "vitest";
import { Suspense, act } from "react";
import { render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { RelayEnvironmentProvider } from "react-relay";
import {
  MockPayloadGenerator,
  createMockEnvironment,
  type RelayMockEnvironment,
} from "relay-test-utils";
import ReportsBadges from "./ReportsBadges";

// relay-test-utils reaches for jest.fn() internally; shim it for vitest.
(globalThis as unknown as { jest: { fn: typeof vi.fn } }).jest = { fn: vi.fn };

vi.mock("../components/useSelectedLocation", () => ({
  default: () => ({
    id: "loc1",
    name: "Test A Unit",
    enabled: true,
    viewerCanEdit: false,
    gamificationEnabled: true,
  }),
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
    reigns: [],
    ...overrides,
  };
}

function renderPage(trophies: ReturnType<typeof trophy>[]) {
  const environment: RelayMockEnvironment = createMockEnvironment();
  render(
    <RelayEnvironmentProvider environment={environment}>
      <MemoryRouter>
        <Suspense fallback={<p>Loading</p>}>
          <ReportsBadges />
        </Suspense>
      </MemoryRouter>
    </RelayEnvironmentProvider>,
  );
  act(() => {
    environment.mock.resolveMostRecentOperation((op) =>
      MockPayloadGenerator.generate(op, {
        Location: () => ({ id: "loc1", people: [], trophies }),
      }),
    );
  });
}

describe("ReportsBadges trophy cabinet", () => {
  it("shows each trophy's holder, and how to win a vacant one", async () => {
    renderPage([
      trophy("kettle", "The Kettle", {
        holder: { id: "p1", firstName: "Bob", lastName: "Brown" },
        valueLabel: "9 busy days",
        heldSince: 1_790_000_000,
        reigns: [
          {
            holder: { firstName: "Alice", lastName: "Anderson" },
            valueLabel: "7 busy days",
            startedAt: 1_780_000_000,
            endedAt: 1_790_000_000,
            endReason: "STOLEN",
            endedBy: { firstName: "Bob", lastName: "Brown" },
            endedByValueLabel: "9 busy days",
          },
        ],
      }),
      trophy("boomerang", "The Boomerang", { minimumLabel: "30 days away" }),
    ]);

    expect(await screen.findByText("Perpetual trophies")).toBeInTheDocument();
    const kettle = screen.getByText("The Kettle").closest("article")!;
    expect(kettle.querySelector("img")).toHaveAttribute(
      "src",
      "/image/badges/by-id/trophy-kettle.svg",
    );
    expect(
      within(kettle).getByRole("link", { name: "Bob Brown" }),
    ).toHaveAttribute("href", "/admin/members/badges/p1");
    expect(within(kettle).getByText(/9 busy days · since/)).toBeInTheDocument();
    expect(within(kettle).getByText("Alice Anderson")).toBeInTheDocument();
    expect(
      within(kettle).getByText(/taken by Bob Brown with 9 busy days/),
    ).toBeInTheDocument();

    const boomerang = screen.getByText("The Boomerang").closest("article")!;
    expect(
      within(boomerang).getByText(
        "Nobody holds it yet. Minimum 30 days away to qualify.",
      ),
    ).toBeInTheDocument();
  });

  it("says why a reign ended in plain words", async () => {
    const reign = {
      holder: { firstName: "Alice", lastName: "Anderson" },
      startedAt: 1_780_000_000,
      endedAt: 1_790_000_000,
      endedBy: { firstName: "Bob", lastName: "Brown" },
    };
    renderPage([
      trophy("chain", "The Chain", {
        reigns: [
          {
            ...reign,
            valueLabel: "33 weeks",
            endReason: "LAPSED",
            endedByValueLabel: "23 weeks",
          },
        ],
      }),
      trophy("boots", "The Boots", {
        reigns: [
          {
            ...reign,
            valueLabel: "30 h",
            endReason: "LAPSED",
            endedByValueLabel: "25 h",
          },
        ],
      }),
      trophy("boomerang", "The Boomerang", {
        reigns: [
          {
            ...reign,
            valueLabel: "412 days away",
            endReason: "AGED_OUT",
            endedByValueLabel: "95 days away",
          },
        ],
      }),
    ]);

    const card = async (name: string) =>
      (await screen.findByText(name)).closest("article")!;
    expect(
      within(await card("The Chain")).getByText(
        /run broken; passed to Bob Brown with 23 weeks/,
      ),
    ).toBeInTheDocument();
    expect(
      within(await card("The Boots")).getByText(
        /fell below the minimum; passed to Bob Brown with 25 h/,
      ),
    ).toBeInTheDocument();
    expect(
      within(await card("The Boomerang")).getByText(
        /record set over a year ago; passed to Bob Brown with 95 days away/,
      ),
    ).toBeInTheDocument();
  });
});

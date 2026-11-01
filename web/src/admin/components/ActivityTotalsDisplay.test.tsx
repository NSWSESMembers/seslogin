import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi } from "vitest";
import { Suspense, act } from "react";
import { render, screen } from "@testing-library/react";
import { RelayEnvironmentProvider } from "react-relay";
import { MockPayloadGenerator, createMockEnvironment } from "relay-test-utils";
import ActivityTotalsDisplay from "./ActivityTotalsDisplay";

// relay-test-utils reaches for jest.fn() internally; shim it for vitest.
(globalThis as unknown as { jest: { fn: typeof vi.fn } }).jest = { fn: vi.fn };

vi.mock("./useUserInfo", () => ({
  useUserInfo: () => ({ disaggregateVirtualPeriods: false }),
}));

// One row per member; the home unit of the row at each index.
const rows = [
  { id: "p1", name: "Alice", homeId: "a", isMemberOf: true },
  { id: "p2", name: "Dana", homeId: "b", isMemberOf: true },
  { id: "p3", name: "Vera", homeId: "c", isMemberOf: false },
];

function row({ id, name, isMemberOf }: (typeof rows)[number]) {
  return {
    person: { id, firstName: name, lastName: "Test", isMemberOf },
    totalTime: 3600,
    totalTimeVirtual: 0,
  };
}

describe("ActivityTotalsDisplay unit labels", () => {
  it("labels a visitor, a member whose home is elsewhere, and leaves a home member plain", async () => {
    const environment = createMockEnvironment();
    render(
      <RelayEnvironmentProvider environment={environment}>
        <Suspense fallback={<p>Loading</p>}>
          <ActivityTotalsDisplay
            locationId="a"
            startTime={0}
            endTime={100}
            categories={[]}
          />
        </Suspense>
      </RelayEnvironmentProvider>,
    );
    act(() => {
      environment.mock.resolveMostRecentOperation((op) =>
        MockPayloadGenerator.generate(op, {
          // The mock answers every nested `location` too, so pick each member's home
          // unit by the row it sits in.
          Location: (ctx) => {
            const index = ctx.path?.indexOf("periodSummaryByMember") ?? -1;
            if (index < 0) {
              return {
                id: "a",
                periodSummaryByMember: rows.map(row),
                periodSummaryByCategory: [],
              };
            }
            const home = rows[Number(ctx.path?.[index + 1])];
            return { id: home.homeId, name: `Home of ${home.name}` };
          },
        }),
      );
    });

    expect(await screen.findByText("Home: Home of Dana")).toBeInTheDocument();
    expect(screen.getByText("Visiting from Home of Vera")).toBeInTheDocument();
    expect(screen.queryByText(/Home of Alice/)).not.toBeInTheDocument();
  });
});

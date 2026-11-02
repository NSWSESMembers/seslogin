import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi } from "vitest";
import { Suspense, act } from "react";
import UserEvent from "@testing-library/user-event";
import { render } from "@testing-library/react";
import { RelayEnvironmentProvider } from "react-relay";
import { MockPayloadGenerator, createMockEnvironment } from "relay-test-utils";
import SettingsDailyChanges from "./SettingsDailyChanges";
import {
  NotifyContext,
  type NotifyContextValue,
} from "../components/useNotify";

// relay-test-utils reaches for jest.fn() internally; shim it for vitest.
(globalThis as unknown as { jest: { fn: typeof vi.fn } }).jest = { fn: vi.fn };

const notify: NotifyContextValue = {
  notify: vi.fn(),
  notifySuccess: vi.fn(),
  notifyError: vi.fn(),
  dismiss: vi.fn(),
};

describe("SettingsDailyChanges", () => {
  it("shows the current opt-ins and saves a new list without touching the others", async () => {
    const environment = createMockEnvironment();
    const screen = render(
      <RelayEnvironmentProvider environment={environment}>
        <NotifyContext.Provider value={notify}>
          <Suspense fallback={<p>Loading</p>}>
            <SettingsDailyChanges />
          </Suspense>
        </NotifyContext.Provider>
      </RelayEnvironmentProvider>,
    );

    act(() => {
      environment.mock.resolveMostRecentOperation((op) =>
        MockPayloadGenerator.generate(op, {
          User: () => ({
            id: "u1",
            emailSummaryLocationIds: ["loc-a"],
            dailyChangesLocationIds: ["loc-a"],
            locations: [
              { id: "loc-a", name: "Alpha Unit" },
              { id: "loc-b", name: "Bravo Unit" },
            ],
          }),
        }),
      );
    });

    const alpha = await screen.findByLabelText("Alpha Unit");
    const bravo = screen.getByLabelText("Bravo Unit");
    expect(alpha).toBeChecked();
    expect(bravo).not.toBeChecked();

    const user = UserEvent.setup();
    await user.click(bravo);
    await user.click(screen.getByRole("button", { name: "Save" }));

    const operation = environment.mock.getMostRecentOperation();
    expect(operation.request.node.params.name).toBe(
      "SettingsDailyChangesMutation",
    );
    // The daily summary list is passed through; only this setting changes.
    expect(operation.request.variables).toEqual({
      dailyLocationIds: ["loc-a"],
      dailyChangesLocationIds: ["loc-a", "loc-b"],
    });
  });
});

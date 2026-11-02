import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi } from "vitest";
import { Suspense, act } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router";
import { RelayEnvironmentProvider } from "react-relay";
import {
  MockPayloadGenerator,
  createMockEnvironment,
  type RelayMockEnvironment,
} from "relay-test-utils";
import LocationEdit from "./LocationEdit";
import { NotifyContext } from "../components/useNotify";

// relay-test-utils reaches for jest.fn() internally; shim it for vitest.
(globalThis as unknown as { jest: { fn: typeof vi.fn } }).jest = { fn: vi.fn };

function renderPage(location: {
  gamificationEnabled: boolean;
  badgeEmailsEnabled: boolean;
}) {
  const environment: RelayMockEnvironment = createMockEnvironment();
  render(
    <RelayEnvironmentProvider environment={environment}>
      <NotifyContext.Provider
        value={{
          notify: vi.fn(),
          notifySuccess: vi.fn(),
          notifyError: vi.fn(),
          dismiss: vi.fn(),
        }}
      >
        <MemoryRouter initialEntries={["/admin/locations/loc1"]}>
          <Suspense fallback={<p>Loading</p>}>
            <Routes>
              <Route
                path="/admin/locations/:locationId"
                element={<LocationEdit />}
              />
            </Routes>
          </Suspense>
        </MemoryRouter>
      </NotifyContext.Provider>
    </RelayEnvironmentProvider>,
  );
  act(() => {
    environment.mock.resolveMostRecentOperation((op) =>
      MockPayloadGenerator.generate(op, {
        Location: () => ({
          id: "loc1",
          name: "Test A Unit",
          enabled: true,
          nitcEnabled: null,
          nitcCompleteOnExport: true,
          ...location,
        }),
      }),
    );
  });
}

describe("LocationEdit congratulations emails", () => {
  it("shows the setting only while gamification is ticked", async () => {
    renderPage({ gamificationEnabled: false, badgeEmailsEnabled: false });

    const gamification = await screen.findByLabelText("Gamification");
    expect(
      screen.queryByLabelText("Congratulations emails"),
    ).not.toBeInTheDocument();

    fireEvent.click(gamification);
    expect(screen.getByLabelText("Congratulations emails")).not.toBeChecked();

    fireEvent.click(gamification);
    expect(
      screen.queryByLabelText("Congratulations emails"),
    ).not.toBeInTheDocument();
  });

  it("shows the stored setting when gamification is on", async () => {
    renderPage({ gamificationEnabled: true, badgeEmailsEnabled: true });

    expect(
      await screen.findByLabelText("Congratulations emails"),
    ).toBeChecked();
  });
});

import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { Suspense, act } from "react";
import UserEvent from "@testing-library/user-event";
import { render } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router";
import { RelayEnvironmentProvider } from "react-relay";
import { MockPayloadGenerator, createMockEnvironment } from "relay-test-utils";
import UserEdit from "./UserEdit";
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

const alice = {
  id: "p-1",
  firstName: "Alice",
  lastName: "Anderson",
  memberNumber: "10000001",
  location: { id: "loc-a", name: "Alpha Unit" },
};

beforeEach(() => {
  vi.clearAllMocks();
});

async function renderPage(person: typeof alice | null) {
  const environment = createMockEnvironment();
  const screen = render(
    <MemoryRouter initialEntries={["/admin/users/u1"]}>
      <RelayEnvironmentProvider environment={environment}>
        <NotifyContext.Provider value={notify}>
          <Suspense fallback={<p>Loading</p>}>
            <Routes>
              <Route path="/admin/users/:userId" element={<UserEdit />} />
            </Routes>
          </Suspense>
        </NotifyContext.Provider>
      </RelayEnvironmentProvider>
    </MemoryRouter>,
  );
  act(() => {
    environment.mock.resolveMostRecentOperation((op) =>
      MockPayloadGenerator.generate(op, {
        User: () => ({
          id: "u1",
          isSuper: false,
          locationGrantIds: [],
          readOnlyLocationGrantIds: [],
          oauthGrants: [],
          person,
        }),
        Query: () => ({
          locations: [
            { id: "loc-a", name: "Alpha Unit" },
            { id: "loc-b", name: "Bravo Unit" },
          ],
        }),
      }),
    );
  });
  await screen.findByText("Current member");
  return { environment, screen };
}

describe("UserEdit member", () => {
  it("sets a member picked from a unit's roster", async () => {
    const { environment, screen } = await renderPage(null);
    const user = UserEvent.setup();
    expect(screen.getByText("None")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Set member" })).toBeDisabled();

    await user.click(screen.getByRole("combobox", { name: "Unit" }));
    await user.click(await screen.findByRole("option", { name: "Bravo Unit" }));

    const peopleQuery = environment.mock.getMostRecentOperation();
    expect(peopleQuery.request.variables.location).toBe("loc-b");
    act(() => {
      environment.mock.resolve(
        peopleQuery,
        MockPayloadGenerator.generate(peopleQuery, {
          Location: () => ({
            people: [
              {
                id: "p-1",
                firstName: "Alice",
                lastName: "Anderson",
                memberNumber: "10000001",
              },
            ],
          }),
        }),
      );
    });

    await user.click(await screen.findByRole("combobox", { name: "Member" }));
    await user.click(
      await screen.findByRole("option", { name: /Alice Anderson/ }),
    );
    await user.click(screen.getByRole("button", { name: "Set member" }));

    const mutation = environment.mock.getMostRecentOperation();
    expect(mutation.request.variables).toEqual({
      userId: "u1",
      personId: "p-1",
    });
    act(() => {
      environment.mock.resolve(
        mutation,
        MockPayloadGenerator.generate(mutation, {
          User: () => ({ id: "u1", person: alice }),
        }),
      );
    });
    expect(notify.notifySuccess).toHaveBeenCalledWith("Member set");
    expect(
      await screen.findByText("Alice Anderson (10000001), Alpha Unit"),
    ).toBeInTheDocument();
  });

  it("clears the current member", async () => {
    const { environment, screen } = await renderPage(alice);
    const user = UserEvent.setup();
    expect(
      screen.getByText("Alice Anderson (10000001), Alpha Unit"),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Clear member" }));

    const mutation = environment.mock
      .getAllOperations()
      .find((op) => op.request.node.params.name === "UserMemberFieldMutation");
    expect(mutation?.request.variables).toEqual({
      userId: "u1",
      personId: null,
    });
  });
});

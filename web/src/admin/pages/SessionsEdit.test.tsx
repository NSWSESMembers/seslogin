import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi } from "vitest";
import { Suspense, act } from "react";
import UserEvent from "@testing-library/user-event";
import { render } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router";
import { RelayEnvironmentProvider } from "react-relay";
import { MockPayloadGenerator, createMockEnvironment } from "relay-test-utils";
import SessionsEdit from "./SessionsEdit";
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

function renderAt(environment: ReturnType<typeof createMockEnvironment>) {
  return render(
    <RelayEnvironmentProvider environment={environment}>
      <NotifyContext.Provider value={notify}>
        <MemoryRouter initialEntries={["/admin/sessions/session1"]}>
          <Suspense fallback={<p>Loading</p>}>
            <Routes>
              <Route path="/admin/sessions" element={<p>Kiosk list</p>} />
              <Route
                path="/admin/sessions/:sessionId"
                element={<SessionsEdit />}
              />
            </Routes>
          </Suspense>
        </MemoryRouter>
      </NotifyContext.Provider>
    </RelayEnvironmentProvider>,
  );
}

describe("SessionsEdit", () => {
  it("shows the mutation's own result on the next visit, not the pre-edit record", async () => {
    const environment = createMockEnvironment();
    const first = renderAt(environment);

    act(() => {
      environment.mock.resolveMostRecentOperation((op) =>
        MockPayloadGenerator.generate(op, {
          Session: () => ({
            name: "Old Name",
            config: {},
            healthcheckUrl: null,
            clientInfo: null,
          }),
        }),
      );
    });

    const nameInput = await first.findByLabelText("Name");
    expect(nameInput).toHaveValue("Old Name");

    const user = UserEvent.setup();
    await user.clear(nameInput);
    await user.type(nameInput, "New Name");
    await user.click(first.getByRole("button", { name: "Save" }));

    // The mutation response is the only thing telling the client what the server now
    // has. If updateSession's selection ever regresses to just `__typename` (its
    // original, buggy shape — see issue #239), this resolves with no `updateSession`
    // fields to normalize, the kiosk record in the store keeps its pre-edit values,
    // and the assertions below catch it instead of silently passing.
    act(() => {
      environment.mock.resolveMostRecentOperation((op) =>
        MockPayloadGenerator.generate(op, {
          Session: () => ({
            name: "New Name",
            config: {},
            healthcheckUrl: null,
          }),
        }),
      );
    });

    await first.findByText("Kiosk list");
    expect(notify.notifySuccess).toHaveBeenCalledWith("Kiosk saved");

    // Leaving the page unmounts SessionsEdit, exactly like the real route swap to
    // the kiosk list. Then re-enter the same kiosk's edit form — the repro from
    // issue #239 — as a genuinely fresh mount rather than a second render of the
    // same instance.
    first.unmount();
    const second = renderAt(environment);

    // `useLazyLoadQuery`'s QueryResource keeps its own response cache for this exact
    // (query, variables) pair independent of the store's invalidation epoch (see
    // RelayErrorBoundary's doc comment for the same gotcha in the retry path), so this
    // fresh mount is not guaranteed to hit the network at all — it may render straight
    // from whatever the store already has. That's fine as long as the store already
    // has the right thing; it must never depend on a second round-trip to correct
    // itself.
    const pending = environment.mock.getAllOperations();
    act(() => {
      for (const op of pending) {
        environment.mock.resolve(
          op,
          MockPayloadGenerator.generate(op, {
            Session: () => ({
              name: "New Name",
              config: {},
              healthcheckUrl: null,
              clientInfo: null,
            }),
          }),
        );
      }
    });

    const nameInputAgain = await second.findByLabelText("Name");
    expect(nameInputAgain).toHaveValue("New Name");
  });
});

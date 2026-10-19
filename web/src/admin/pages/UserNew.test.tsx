import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { Suspense, act } from "react";
import UserEvent from "@testing-library/user-event";
import { render } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { RelayEnvironmentProvider } from "react-relay";
import { MockPayloadGenerator, createMockEnvironment } from "relay-test-utils";
import UserNew from "./UserNew";
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

beforeEach(() => {
  vi.clearAllMocks();
});

async function renderPage(existingEmails: string[] = []) {
  const environment = createMockEnvironment();
  const screen = render(
    <MemoryRouter>
      <RelayEnvironmentProvider environment={environment}>
        <NotifyContext.Provider value={notify}>
          <Suspense fallback={<p>Loading</p>}>
            <UserNew />
          </Suspense>
        </NotifyContext.Provider>
      </RelayEnvironmentProvider>
    </MemoryRouter>,
  );
  act(() => {
    environment.mock.resolveMostRecentOperation((op) =>
      MockPayloadGenerator.generate(op, {
        Query: () => ({
          locations: [],
          users: existingEmails.map((email) => ({ email })),
        }),
      }),
    );
  });
  await screen.findByLabelText("Email");
  return { environment, screen };
}

describe("UserNew", () => {
  it("creates one user per email with the same permissions", async () => {
    const { environment, screen } = await renderPage();
    const user = UserEvent.setup();

    await user.type(
      screen.getByLabelText("Email"),
      "a@example.com, b@example.com{enter}c@example.com",
    );
    await user.click(screen.getByLabelText("Super"));
    await user.click(screen.getByRole("button", { name: "Create 3 users" }));

    const created: string[] = [];
    for (let i = 0; i < 3; i++) {
      await vi.waitFor(() =>
        expect(environment.mock.getAllOperations()).toHaveLength(1),
      );
      const op = environment.mock.getMostRecentOperation();
      expect(op.request.variables.isSuper).toBe(true);
      created.push(op.request.variables.email as string);
      await act(async () => {
        environment.mock.resolve(op, MockPayloadGenerator.generate(op));
      });
    }

    expect(created).toEqual([
      "a@example.com",
      "b@example.com",
      "c@example.com",
    ]);
    await vi.waitFor(() =>
      expect(notify.notifySuccess).toHaveBeenCalledWith("3 users created"),
    );
  });

  it("refuses the batch if any email is already a user", async () => {
    const { environment, screen } = await renderPage(["B@example.com"]);
    const user = UserEvent.setup();

    await user.type(
      screen.getByLabelText("Email"),
      "a@example.com b@example.com",
    );
    await user.click(screen.getByRole("button", { name: "Create 2 users" }));

    expect(notify.notify).toHaveBeenCalledWith("Already a user: b@example.com");
    expect(environment.mock.getAllOperations()).toHaveLength(0);
  });

  it("leaves only the failed emails to retry", async () => {
    const { environment, screen } = await renderPage();
    const user = UserEvent.setup();

    await user.type(
      screen.getByLabelText("Email"),
      "a@example.com b@example.com",
    );
    await user.click(screen.getByRole("button", { name: "Create 2 users" }));

    await vi.waitFor(() =>
      expect(environment.mock.getAllOperations()).toHaveLength(1),
    );
    await act(async () => {
      environment.mock.rejectMostRecentOperation(new Error("boom"));
    });
    await vi.waitFor(() =>
      expect(environment.mock.getAllOperations()).toHaveLength(1),
    );
    await act(async () => {
      const op = environment.mock.getMostRecentOperation();
      environment.mock.resolve(op, MockPayloadGenerator.generate(op));
    });

    await vi.waitFor(() =>
      expect(screen.getByLabelText("Email")).toHaveValue("a@example.com"),
    );
    expect(notify.notifyError).toHaveBeenCalledWith(
      expect.any(Error),
      "Couldn't create a@example.com",
    );
    expect(notify.notifySuccess).toHaveBeenCalledWith(
      "Created 1 of 2 users. The rest are left below to retry.",
    );
  });
});

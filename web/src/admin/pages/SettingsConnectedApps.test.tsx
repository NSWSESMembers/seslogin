import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi } from "vitest";
import { Suspense, act } from "react";
import UserEvent from "@testing-library/user-event";
import { render } from "@testing-library/react";
import { RelayEnvironmentProvider } from "react-relay";
import { MockPayloadGenerator, createMockEnvironment } from "relay-test-utils";
import SettingsConnectedApps from "./SettingsConnectedApps";
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
        <Suspense fallback={<p>Loading</p>}>
          <SettingsConnectedApps />
        </Suspense>
      </NotifyContext.Provider>
    </RelayEnvironmentProvider>,
  );
}

describe("SettingsConnectedApps", () => {
  it("shows the empty state when there are no connected apps", async () => {
    const environment = createMockEnvironment();
    const screen = renderAt(environment);

    act(() => {
      environment.mock.resolveMostRecentOperation((op) =>
        MockPayloadGenerator.generate(op, {
          User: () => ({ oauthGrants: [] }),
        }),
      );
    });

    expect(
      await screen.findByText("No apps connected yet."),
    ).toBeInTheDocument();
  });

  it("lists connected apps and revokes one on confirm", async () => {
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    const environment = createMockEnvironment();
    const screen = renderAt(environment);

    act(() => {
      environment.mock.resolveMostRecentOperation((op) =>
        MockPayloadGenerator.generate(op, {
          OauthGrant: () => ({
            id: "grant-1",
            clientName: "Claude Code",
            redirectHost: "claude.ai",
            createdAt: 1_700_000_000,
            lastUsedAt: 1_700_100_000,
            refreshExpiresAt: 1_800_000_000,
          }),
        }),
      );
    });

    expect(await screen.findByText("Claude Code")).toBeInTheDocument();
    expect(screen.getByText("claude.ai")).toBeInTheDocument();

    const user = UserEvent.setup();
    await user.click(screen.getByRole("button", { name: "Disconnect" }));
    expect(confirmSpy).toHaveBeenCalled();

    act(() => {
      environment.mock.resolveMostRecentOperation((op) =>
        MockPayloadGenerator.generate(op, {
          Boolean: () => true,
        }),
      );
    });

    expect(notify.notifySuccess).toHaveBeenCalledWith(
      'Disconnected "Claude Code"',
    );

    // The revoke bumps the refetch key, which refires the list query — resolve
    // it with an empty list, mirroring the grant now being gone server-side.
    act(() => {
      environment.mock.resolveMostRecentOperation((op) =>
        MockPayloadGenerator.generate(op, {
          User: () => ({ oauthGrants: [] }),
        }),
      );
    });

    expect(
      await screen.findByText("No apps connected yet."),
    ).toBeInTheDocument();

    confirmSpy.mockRestore();
  });
});

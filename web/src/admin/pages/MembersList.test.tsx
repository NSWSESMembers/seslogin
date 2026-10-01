import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi } from "vitest";
import { Suspense, act } from "react";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { RelayEnvironmentProvider } from "react-relay";
import { MockPayloadGenerator, createMockEnvironment } from "relay-test-utils";
import MembersList from "./MembersList";
import { NotifyContext } from "../components/useNotify";

// relay-test-utils reaches for jest.fn() internally; shim it for vitest.
(globalThis as unknown as { jest: { fn: typeof vi.fn } }).jest = { fn: vi.fn };

const selected = vi.hoisted(() => ({
  viewerCanEdit: true,
}));

vi.mock("../components/useSelectedLocation", () => ({
  default: () => ({
    id: "loc1",
    name: "HQ",
    enabled: true,
    viewerCanEdit: selected.viewerCanEdit,
  }),
}));
vi.mock("../components/useUserInfo", () => ({
  useUserInfo: () => ({ isDev: false }),
}));

function renderList(viewerCanEdit: boolean) {
  selected.viewerCanEdit = viewerCanEdit;
  const environment = createMockEnvironment();
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
        <MemoryRouter>
          <Suspense fallback={<p>Loading</p>}>
            <MembersList />
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
          sesApiHeadquartersId: "hq",
          lastSuccessfulMemberSync: 1,
          people: [
            {
              id: "p1",
              firstName: "Alice",
              lastName: "Anderson",
              memberNumber: "1",
              sesApiPersonId: null,
              missingSince: null,
            },
          ],
        }),
      }),
    );
  });
}

describe("MembersList", () => {
  it("offers Edit, Delete and Sync now to editors", async () => {
    renderList(true);
    expect(await screen.findByText("Alice Anderson")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Edit" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Delete" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Sync now" }),
    ).toBeInTheDocument();
  });

  it("hides every write control from read only users but keeps Activity", async () => {
    renderList(false);
    expect(await screen.findByText("Alice Anderson")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Activity" })).toBeInTheDocument();
    expect(
      screen.queryByRole("link", { name: "Edit" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Delete" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Sync now" }),
    ).not.toBeInTheDocument();
  });
});

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

function renderList(
  viewerCanEdit: boolean,
  sesApiPersonId: string | null = null,
  person: {
    locationId: string;
    primaryName: string;
    viewerCanEdit: boolean;
  } = { locationId: "loc1", primaryName: "HQ", viewerCanEdit },
) {
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
          // The mock resolver also answers each member's nested `location`, so this
          // is the primary unit's name as the row sees it.
          name: person.primaryName,
          sesApiHeadquartersId: "hq",
          lastSuccessfulMemberSync: 1,
          people: [
            {
              id: "p1",
              firstName: "Alice",
              lastName: "Anderson",
              memberNumber: "1",
              sesApiPersonId,
              missingSince: null,
              locationId: person.locationId,
              location: { id: person.locationId, name: person.primaryName },
              viewerCanEdit: person.viewerCanEdit,
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

describe("MembersList secondary members", () => {
  it("names the primary unit and offers no Edit or Delete, even to an editor of this unit", async () => {
    renderList(true, null, {
      locationId: "loc2",
      primaryName: "Other Unit",
      viewerCanEdit: false,
    });
    expect(await screen.findByText("Alice Anderson")).toBeInTheDocument();
    expect(screen.getByText("Primary: Other Unit")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Activity" })).toBeInTheDocument();
    expect(
      screen.queryByRole("link", { name: "Edit" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Delete" }),
    ).not.toBeInTheDocument();
    // The unit's own Sync now is unaffected.
    expect(
      screen.getByRole("button", { name: "Sync now" }),
    ).toBeInTheDocument();
  });

  it("shows no primary unit label for a primary-unit member", async () => {
    renderList(true);
    expect(await screen.findByText("Alice Anderson")).toBeInTheDocument();
    expect(screen.queryByText(/^Primary:/)).not.toBeInTheDocument();
  });

  it("follows the member's own viewerCanEdit, not the unit's", async () => {
    renderList(false, null, {
      locationId: "loc1",
      primaryName: "HQ",
      viewerCanEdit: true,
    });
    expect(await screen.findByText("Alice Anderson")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Edit" })).toBeInTheDocument();
  });
});

describe("MembersList details", () => {
  it("offers Details only for synced members, even to read only users", async () => {
    renderList(false, null);
    expect(await screen.findByText("Alice Anderson")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Details" }),
    ).not.toBeInTheDocument();
  });

  it("shows Details first for a synced member", async () => {
    renderList(false, "55");
    expect(await screen.findByText("Alice Anderson")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Details" })).toBeInTheDocument();
  });

  it("offers Unit details next to the last sync time", async () => {
    renderList(false);
    expect(
      await screen.findByRole("button", { name: "Unit details" }),
    ).toBeInTheDocument();
  });
});

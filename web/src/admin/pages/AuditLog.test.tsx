import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi } from "vitest";
import { Suspense, act } from "react";
import { render, screen, within } from "@testing-library/react";
import UserEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { RelayEnvironmentProvider } from "react-relay";
import {
  MockPayloadGenerator,
  createMockEnvironment,
  type RelayMockEnvironment,
} from "relay-test-utils";
import AuditLog from "./AuditLog";
import { NotifyContext } from "../components/useNotify";

// relay-test-utils reaches for jest.fn() internally; shim it for vitest.
(globalThis as unknown as { jest: { fn: typeof vi.fn } }).jest = { fn: vi.fn };

const viewer = vi.hoisted(() => ({ isSuper: false }));

vi.mock("../components/useSelectedLocation", () => ({
  default: () => ({
    id: "loc1",
    name: "HQ",
    enabled: true,
    viewerCanEdit: false,
  }),
}));
vi.mock("../components/useUserInfo", () => ({
  useUserInfo: () => ({ isSuper: viewer.isSuper, isDev: false }),
}));

function entry(n: number, overrides: Record<string, unknown> = {}) {
  return {
    id: `entry${n}`,
    timestamp: 1_700_000_000 + n,
    action: "UPDATE",
    entityType: "PERSON",
    entityId: `person${n}`,
    entityLabel: `Member ${n}`,
    location: { id: "loc1", name: "HQ" },
    actor: {
      kind: "USER",
      actorId: `user${n}`,
      via: null,
      label: "admin@example.test",
    },
    ip: null,
    changes: [
      { field: "first_name", before: "Alice", after: "Alicia" },
      { field: "missing_since", before: null, after: "1700000000" },
    ],
    ...overrides,
  };
}

function connection(
  entries: ReturnType<typeof entry>[],
  hasNextPage: boolean,
  endCursor: string | null,
) {
  return {
    edges: entries.map((node) => ({ node, cursor: node.id })),
    pageInfo: { hasNextPage, endCursor },
  };
}

function renderPage(isSuper = false) {
  viewer.isSuper = isSuper;
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
            <AuditLog />
          </Suspense>
        </MemoryRouter>
      </NotifyContext.Provider>
    </RelayEnvironmentProvider>,
  );
  return environment;
}

function resolveLocation(
  environment: RelayMockEnvironment,
  entries: ReturnType<typeof entry>[],
  hasNextPage = false,
  endCursor: string | null = null,
) {
  act(() => {
    environment.mock.resolveMostRecentOperation((op) =>
      MockPayloadGenerator.generate(op, {
        Location: () => ({
          id: "loc1",
          auditLog: connection(entries, hasNextPage, endCursor),
        }),
      }),
    );
  });
}

function resolveRoot(
  environment: RelayMockEnvironment,
  entries: ReturnType<typeof entry>[],
) {
  act(() => {
    environment.mock.resolveMostRecentOperation((op) =>
      MockPayloadGenerator.generate(op, {
        Query: () => ({ auditLog: connection(entries, false, null) }),
      }),
    );
  });
}

describe("AuditLog", () => {
  it("lists entries and expands one to show its changes", async () => {
    const user = UserEvent.setup();
    const environment = renderPage();
    resolveLocation(environment, [
      entry(1),
      entry(2, {
        action: "CREATE",
        entityType: "SESSION",
        entityLabel: "Front desk",
        actor: {
          kind: "SYSTEM",
          actorId: "member-sync",
          via: null,
          label: null,
        },
        changes: [],
      }),
    ]);

    expect(await screen.findByText("Member 1")).toBeInTheDocument();
    expect(screen.getByText("admin@example.test")).toBeInTheDocument();
    expect(screen.getByText("Updated")).toBeInTheDocument();
    expect(screen.getByText("Created")).toBeInTheDocument();
    expect(screen.getByText("Front desk")).toBeInTheDocument();
    expect(
      within(screen.getByRole("table")).getByText("Kiosk"),
    ).toBeInTheDocument();
    expect(screen.getByText("System: member-sync")).toBeInTheDocument();
    expect(screen.queryByText("first_name")).not.toBeInTheDocument();
    // The actor's ID is a user/kiosk ID; it must not become a store record that
    // clobbers that user's own record.
    expect(environment.getStore().getSource().get("user1")).toBeUndefined();
    // No location column outside the all-locations view.
    expect(
      screen.queryByRole("columnheader", { name: "Location" }),
    ).not.toBeInTheDocument();

    await user.click(screen.getAllByRole("button", { name: "Details" })[0]);
    expect(screen.getByText("first_name")).toBeInTheDocument();
    expect(screen.getByText("Alicia")).toBeInTheDocument();
    // An absent "before" is a dash, and epoch fields are shown as dates.
    expect(screen.getByText("—")).toBeInTheDocument();
    expect(screen.queryByText("1700000000")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Hide details" }));
    expect(screen.queryByText("first_name")).not.toBeInTheDocument();
  });

  it("shows a message when there are no entries", async () => {
    const environment = renderPage();
    resolveLocation(environment, []);
    expect(
      await screen.findByText("No changes have been recorded yet."),
    ).toBeInTheDocument();
  });

  it("does not offer the all-locations toggle to non-super users", async () => {
    const environment = renderPage(false);
    resolveLocation(environment, [entry(1)]);
    expect(await screen.findByText("Member 1")).toBeInTheDocument();
    expect(screen.queryByLabelText("All locations")).not.toBeInTheDocument();
  });

  it("lets super users switch to the all-locations log", async () => {
    const user = UserEvent.setup();
    const environment = renderPage(true);
    resolveLocation(environment, [entry(1)]);
    expect(await screen.findByText("Member 1")).toBeInTheDocument();

    await user.click(screen.getByLabelText("All locations"));

    const op = environment.mock.getMostRecentOperation();
    expect(op.request.variables).toMatchObject({ all: true });
    resolveRoot(environment, [
      entry(7, {
        entityType: "CATEGORY",
        entityLabel: "Training",
        location: null,
        ip: "203.0.113.9",
        actor: {
          kind: "USER",
          actorId: "rootuser",
          via: "oauth_grant:g1",
          label: "root@example.test",
        },
      }),
      entry(8, { location: { id: "loc2", name: "Other Unit" } }),
    ]);

    expect(await screen.findByText("Training")).toBeInTheDocument();
    expect(
      screen.getByRole("columnheader", { name: "Location" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Other Unit")).toBeInTheDocument();
    expect(screen.getByText("via oauth_grant:g1")).toBeInTheDocument();
    expect(screen.queryByText("Member 1")).not.toBeInTheDocument();

    await user.click(screen.getAllByRole("button", { name: "Details" })[0]);
    expect(screen.getByText(/IP 203\.0\.113\.9/)).toBeInTheDocument();
  });

  it("filters by entity type on the server and starts again", async () => {
    const user = UserEvent.setup();
    const environment = renderPage();
    resolveLocation(environment, [entry(1)]);
    expect(await screen.findByText("Member 1")).toBeInTheDocument();

    await user.selectOptions(screen.getByRole("combobox"), "Kiosk");

    expect(
      environment.mock.getMostRecentOperation().request.variables,
    ).toMatchObject({ entityType: "SESSION", after: null, all: false });
  });

  it("loads more with the cursor and appends the rows", async () => {
    const user = UserEvent.setup();
    const environment = renderPage();
    resolveLocation(environment, [entry(1)], true, "cursor1");
    expect(await screen.findByText("Member 1")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Load More" }));

    expect(
      environment.mock.getMostRecentOperation().request.variables,
    ).toMatchObject({ after: "cursor1", first: 50 });
    resolveLocation(environment, [entry(2)], false, null);

    const table = await screen.findByRole("table");
    expect(await within(table).findByText("Member 2")).toBeInTheDocument();
    expect(within(table).getByText("Member 1")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Load More" }),
    ).not.toBeInTheDocument();
  });
});

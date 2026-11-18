import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi } from "vitest";
import { Suspense, act } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { RelayEnvironmentProvider } from "react-relay";
import { MockPayloadGenerator, createMockEnvironment } from "relay-test-utils";
import MemberDetailsDialog from "./MemberDetailsDialog";
import LocationDetailsDialog from "./LocationDetailsDialog";

// relay-test-utils reaches for jest.fn() internally; shim it for vitest.
(globalThis as unknown as { jest: { fn: typeof vi.fn } }).jest = { fn: vi.fn };

function renderDialog(
  ui: React.ReactNode,
  resolvers: Parameters<typeof MockPayloadGenerator.generate>[1],
) {
  const environment = createMockEnvironment();
  render(
    <RelayEnvironmentProvider environment={environment}>
      <Suspense fallback={<p>Loading</p>}>{ui}</Suspense>
    </RelayEnvironmentProvider>,
  );
  expect(screen.getByText("Loading...")).toBeInTheDocument();
  act(() => {
    environment.mock.resolveMostRecentOperation((op) =>
      MockPayloadGenerator.generate(op, resolvers),
    );
  });
}

const person = {
  id: "p1",
  firstName: "Bob",
  lastName: "Brown",
  preferredName: "Bobby",
  registrationNumber: "10000002",
  sesApiPersonId: "55",
  email: "bob@example.com",
  units: [
    {
      entityId: "9",
      entityName: "Test A Unit",
      entityCode: "TST-A",
      isPrimary: true,
    },
    {
      entityId: "10",
      entityName: "Test B Unit",
      entityCode: null,
      isPrimary: false,
    },
  ],
};

describe("MemberDetailsDialog", () => {
  it("shows the synced details and closes on Close", async () => {
    const onClose = vi.fn();
    renderDialog(
      <MemberDetailsDialog
        personId="p1"
        personName="Bob Brown"
        onClose={onClose}
      />,
      { Person: () => person },
    );
    expect(await screen.findByText(/prefers Bobby/)).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "bob@example.com" }),
    ).toHaveAttribute("href", "mailto:bob@example.com");
    for (const gone of ["Mobile", "Other phones"]) {
      expect(screen.queryByText(gone)).not.toBeInTheDocument();
    }
    expect(
      screen.queryByRole("link", { name: /^\+61/ }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Qualifications")).not.toBeInTheDocument();
    const units = screen.getAllByRole("listitem");
    expect(units.at(-2)).toHaveTextContent("Test A Unit (TST-A)Primary");
    expect(units.at(-1)).toHaveTextContent(/^Test B Unit$/);
    expect(screen.getAllByText("Primary")).toHaveLength(1);

    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("closes on Escape", async () => {
    const onClose = vi.fn();
    renderDialog(
      <MemberDetailsDialog
        personId="p1"
        personName="Bob Brown"
        onClose={onClose}
      />,
      { Person: () => person },
    );
    await screen.findByText("Test B Unit");
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("says so when there is nothing recorded and never prints null", async () => {
    renderDialog(
      <MemberDetailsDialog
        personId="p1"
        personName="Bob Brown"
        onClose={vi.fn()}
      />,
      {
        Person: () => ({
          ...person,
          preferredName: null,
          email: null,
          units: [],
        }),
      },
    );
    expect(await screen.findByText("Units")).toBeInTheDocument();
    expect(screen.getAllByText("None recorded")).toHaveLength(1);
    expect(screen.queryByText(/prefers/)).not.toBeInTheDocument();
    expect(screen.queryByText(/null/)).not.toBeInTheDocument();
  });
});

const location = {
  id: "loc1",
  name: "Test A Unit",
  sesApiHeadquartersId: "900001",
  code: "TST-A",
  sesDirectoryEntityId: "70001",
};

describe("LocationDetailsDialog", () => {
  it("shows the unit details and closes on Close", async () => {
    const onClose = vi.fn();
    renderDialog(
      <LocationDetailsDialog
        locationId="loc1"
        locationName="Test A Unit"
        onClose={onClose}
      />,
      {
        // The zone is a Location too, so the path tells it apart from the unit.
        Location: ({ path }) =>
          path?.at(-1) === "zone" ? { name: "Test Zone" } : location,
      },
    );
    expect(await screen.findByText("TST-A")).toBeInTheDocument();
    expect(screen.getByText("Test Zone")).toBeInTheDocument();
    expect(screen.getByText("900001")).toBeInTheDocument();
    expect(screen.getByText("70001")).toBeInTheDocument();
    for (const gone of [
      "Coordinates",
      "Address",
      "Postal address",
      "Email",
      "Phone",
      "Accreditations",
    ]) {
      expect(screen.queryByText(gone)).not.toBeInTheDocument();
    }
    expect(screen.queryByRole("link")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("shows placeholders for missing details", async () => {
    renderDialog(
      <LocationDetailsDialog
        locationId="loc1"
        locationName="Test A Unit"
        onClose={vi.fn()}
      />,
      {
        Location: () => ({
          ...location,
          code: null,
          zone: null,
          sesDirectoryEntityId: null,
        }),
      },
    );
    expect(await screen.findByText("Code")).toBeInTheDocument();
    expect(screen.getAllByText("—")).toHaveLength(3);
    expect(screen.queryByText(/null/)).not.toBeInTheDocument();
  });
});

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
  mobile: "+61400000123",
  phones: [
    { number: "+61400000123", types: ["SMS"] },
    { number: "+61255500142", types: ["Home Telephone"] },
  ],
  capabilityTags: [
    { id: "1", name: "First Aid", description: "Provide first aid" },
  ],
  units: [
    {
      entityId: "9",
      entityName: "Test A Unit",
      entityCode: "TST-A",
      zone: "Test Zone",
      membershipType: "Member",
      ranks: ["Team Leader"],
      roles: ["Operator"],
      positions: [],
      isPrimary: true,
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
    expect(
      screen.getByRole("link", { name: "+61400000123" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "+61255500142" }),
    ).toBeInTheDocument();
    expect(screen.getByText("(Home Telephone)")).toBeInTheDocument();
    expect(screen.getByText("First Aid")).toBeInTheDocument();
    expect(screen.getByText("Test A Unit")).toBeInTheDocument();
    expect(screen.getByText("Primary")).toBeInTheDocument();
    expect(screen.getByText("Team Leader")).toBeInTheDocument();
    expect(screen.queryByText("Positions")).not.toBeInTheDocument();

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
    await screen.findByText("First Aid");
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
          mobile: null,
          phones: [],
          capabilityTags: [],
          units: [],
        }),
      },
    );
    expect(await screen.findByText("Qualifications")).toBeInTheDocument();
    expect(screen.getAllByText("None recorded")).toHaveLength(2);
    expect(screen.queryByText(/prefers/)).not.toBeInTheDocument();
    expect(screen.queryByText("Other phones")).not.toBeInTheDocument();
    expect(screen.queryByText(/null/)).not.toBeInTheDocument();
  });
});

const location = {
  id: "loc1",
  name: "Test A Unit",
  sesApiHeadquartersId: "900001",
  code: "TST-A",
  latitude: -33.5,
  longitude: 151.25,
  sesDirectoryEntityId: "70001",
  address: {
    line1: "1 Example Street",
    line2: null,
    suburb: "Exampleville",
    state: "NSW",
    postcode: "2000",
    formatted: null,
  },
  postalAddress: null,
  email: "unit@example.com",
  phone: "02 5550 0100",
  accreditations: [
    {
      type: "Flood Rescue",
      status: "Accredited",
      number: "FR-1",
      note: null,
    },
  ],
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
      { Location: () => location },
    );
    expect(
      await screen.findByText("1 Example Street, Exampleville NSW 2000"),
    ).toBeInTheDocument();
    expect(screen.getByText("TST-A")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "-33.5, 151.25" })).toHaveAttribute(
      "href",
      "https://www.google.com/maps?q=-33.5,151.25",
    );
    expect(screen.getByText("Flood Rescue")).toBeInTheDocument();
    expect(screen.getByText(/FR-1/)).toBeInTheDocument();

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
          latitude: null,
          longitude: null,
          address: null,
          email: null,
          phone: null,
          accreditations: [],
        }),
      },
    );
    expect(await screen.findByText("Accreditations")).toBeInTheDocument();
    expect(screen.getByText("None recorded")).toBeInTheDocument();
    expect(
      screen.queryByRole("link", { name: /maps/ }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText(/null/)).not.toBeInTheDocument();
  });
});

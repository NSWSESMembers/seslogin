import "@testing-library/jest-dom/vitest";
import { describe, it, expect } from "vitest";
import { useState } from "react";
import UserEvent from "@testing-library/user-event";
import { render, screen } from "@testing-library/react";
import LocationRolesField from "./LocationRolesField";
import {
  joinLocationRoles,
  splitLocationRoles,
  type LocationRoleEntry,
} from "./locationRoles";

const locations = [
  { id: "a", name: "Alpha Unit" },
  { id: "b", name: "Bravo Unit" },
  { id: "c", name: "Charlie Unit" },
];

function latest(): LocationRoleEntry[] {
  return JSON.parse(screen.getByTestId("value").textContent ?? "[]");
}

function Harness(props: { initial?: LocationRoleEntry[] }) {
  const [value, setValue] = useState<LocationRoleEntry[]>(props.initial ?? []);
  return (
    <>
      <output data-testid="value">{JSON.stringify(value)}</output>
      <LocationRolesField
        id="locations"
        locations={locations}
        value={value}
        onChange={setValue}
      />
    </>
  );
}

async function addLocation(
  user: ReturnType<typeof UserEvent.setup>,
  name: string,
) {
  await user.click(screen.getByRole("combobox", { name: "Add location" }));
  await user.click(await screen.findByRole("option", { name }));
}

describe("LocationRolesField", () => {
  it("adds a location as Admin", async () => {
    const user = UserEvent.setup();
    render(<Harness />);
    await addLocation(user, "Bravo Unit");
    expect(latest()).toEqual([{ locationId: "b", role: "admin" }]);
    expect(
      screen.getByRole("combobox", { name: "Role at Bravo Unit" }),
    ).toHaveValue("admin");
    expect(screen.getByText("Bravo Unit")).toBeInTheDocument();
  });

  it("does not offer locations that are already listed", async () => {
    const user = UserEvent.setup();
    render(<Harness initial={[{ locationId: "a", role: "admin" }]} />);
    await user.click(screen.getByRole("combobox", { name: "Add location" }));
    expect(
      screen.queryByRole("option", { name: "Alpha Unit" }),
    ).not.toBeInTheDocument();
    expect(
      await screen.findByRole("option", { name: "Bravo Unit" }),
    ).toBeInTheDocument();
  });

  it("changes a row's role", async () => {
    const user = UserEvent.setup();
    render(<Harness initial={[{ locationId: "a", role: "admin" }]} />);
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Role at Alpha Unit" }),
      "Read only",
    );
    expect(latest()).toEqual([{ locationId: "a", role: "readOnly" }]);
  });

  it("removes a row", async () => {
    const user = UserEvent.setup();
    render(
      <Harness
        initial={[
          { locationId: "a", role: "admin" },
          { locationId: "b", role: "readOnly" },
        ]}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Remove Alpha Unit" }));
    expect(latest()).toEqual([{ locationId: "b", role: "readOnly" }]);
    expect(screen.queryByText("Alpha Unit")).not.toBeInTheDocument();
  });
});

describe("splitLocationRoles / joinLocationRoles", () => {
  const entries: LocationRoleEntry[] = [
    { locationId: "a", role: "admin" },
    { locationId: "b", role: "readOnly" },
    { locationId: "c", role: "admin" },
  ];

  it("splits entries into admin and read-only id lists", () => {
    expect(splitLocationRoles(entries)).toEqual({
      locationGrants: ["a", "c"],
      readOnlyLocationGrants: ["b"],
    });
  });

  it("round-trips through the two lists", () => {
    const { locationGrants, readOnlyLocationGrants } =
      splitLocationRoles(entries);
    expect(
      splitLocationRoles(
        joinLocationRoles(locationGrants, readOnlyLocationGrants),
      ),
    ).toEqual({ locationGrants, readOnlyLocationGrants });
  });
});

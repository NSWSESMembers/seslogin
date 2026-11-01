import { describe, it, expect } from "vitest";
import { memberUnitLabel } from "./memberUnitLabel";

const home = { id: "b", name: "Unit B" };

describe("memberUnitLabel", () => {
  it("shows nothing for a member whose home is the viewed unit", () => {
    expect(memberUnitLabel(true, home.id, home.name, "b")).toBeNull();
  });

  it("calls a member of the viewed unit by their home.id, home.name, never a visitor", () => {
    expect(memberUnitLabel(true, home.id, home.name, "a")).toBe("Home: Unit B");
  });

  it("calls someone who is not a member of the viewed unit a visitor", () => {
    expect(memberUnitLabel(false, home.id, home.name, "a")).toBe(
      "Visiting from Unit B",
    );
  });
});

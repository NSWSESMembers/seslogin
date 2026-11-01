import { describe, it, expect } from "vitest";
import { memberUnitLabel } from "./memberUnitLabel";

const primary = { id: "b", name: "Unit B" };

describe("memberUnitLabel", () => {
  it("shows nothing for a member whose primary unit is the viewed unit", () => {
    expect(memberUnitLabel(true, primary.id, primary.name, "b")).toBeNull();
  });

  it("calls a member of the viewed unit by their primary.id, primary.name, never a visitor", () => {
    expect(memberUnitLabel(true, primary.id, primary.name, "a")).toBe(
      "Primary: Unit B",
    );
  });

  it("calls someone who is not a member of the viewed unit a visitor", () => {
    expect(memberUnitLabel(false, primary.id, primary.name, "a")).toBe(
      "Visiting from Unit B",
    );
  });
});

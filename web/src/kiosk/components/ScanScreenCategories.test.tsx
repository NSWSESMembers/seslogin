import { render, screen } from "@testing-library/react";
import ScanScreenCategories from "./ScanScreenCategories";
import { describe, expect, it, vitest } from "vitest";
import UserEvent from "@testing-library/user-event";

// The inlined category icon (see CategoryIcon) is decorative but, unlike the
// `<img>` it replaced, is real DOM content - some icons even carry a badge
// glyph (e.g. Flood Operator L1's "1"). Strip it so this reads the same
// button label text as before.
function labelText(element: Element): string {
  const clone = element.cloneNode(true) as Element;
  clone.querySelectorAll('[data-icon="category"]').forEach((el) => el.remove());
  return clone.textContent ?? "";
}

describe("ScanScreenCategories", () => {
  it("renders the list of categories in alphabetical order", () => {
    const onSelect = vitest.fn();
    render(
      <ScanScreenCategories
        uuid={null}
        screenPosition={"center"}
        onSelectCategory={onSelect}
        smallCategories={false}
      />,
    );
    const categoryElements = screen.getAllByRole("button");
    expect(categoryElements).toHaveLength(10);
    expect(categoryElements.map(labelText)).toEqual([
      "Accredited Rescue Role",
      "Assessor",
      "Combat Roles",
      "Community Ed. & Media",
      "Other",
      "Support Roles",
      "Trainer",
      "Training",
      "Workshop - Participant",
      "Workshop - Trainer",
    ]);
  });

  it("renders subcategories in alphabetical order", async () => {
    const onSelect = vitest.fn();
    const user = UserEvent.setup();
    render(
      <ScanScreenCategories
        uuid={null}
        screenPosition={"center"}
        onSelectCategory={onSelect}
        smallCategories={false}
      />,
    );
    const trainingCategory = screen.getByText("Training");
    await user.click(trainingCategory);

    const categoryElements = screen.getAllByRole("button");
    expect(categoryElements.map(labelText)).toEqual([
      "← Categories",
      "AIIMS",
      "Beacon",
      "Chain Saw",
      "Critical Incident Support",
      "Drive Operational Vehicles",
      "Field Core Skills",
      "First Aid",
      "Fit for Role",
      "Flood Operator L1",
      "Flood Operator L2 (Boat)",
      "Flood Operator L3 (SWR)",
      "Industrial & Domestic Rescue",
      "Job Ready",
      "Land Search",
      "Large Animal Rescue",
      "Map & Navigation",
      "Operate Comms. Equip.",
      "Other",
      "PIARO",
      "RCR",
      "Storm & Water",
      "Traffic Safety",
      "USAR",
      "VR",
    ]);
  });
});

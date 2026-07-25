import "@testing-library/jest-dom/vitest";
import { describe, it, expect } from "vitest";
import UserEvent from "@testing-library/user-event";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router";
import SubmenuBar from "./SubmenuBar";

function renderSubmenuBar(initialPath: string, isSuper = false) {
  return render(
    <MemoryRouter initialEntries={[initialPath]}>
      <Routes>
        <Route path="*" element={<SubmenuBar isSuper={isSuper} />} />
      </Routes>
    </MemoryRouter>,
  );
}

// jsdom doesn't load real CSS, so Tailwind's `hidden` class has no layout
// effect here — assert on the class toggle directly instead of visibility.
function linksContainer() {
  return document.getElementById("admin-submenu-links");
}

describe("SubmenuBar", () => {
  it("renders nothing for a section without a submenu", () => {
    const { container } = renderSubmenuBar("/admin/reports");
    expect(container).toBeEmptyDOMElement();
  });

  it("shows the current page as the mobile toggle's label", () => {
    renderSubmenuBar("/admin/activity/totals");
    expect(screen.getByRole("button", { name: "Totals" })).toBeInTheDocument();
  });

  it("hides the links behind the mobile toggle until opened", async () => {
    const user = UserEvent.setup();
    renderSubmenuBar("/admin/activity");

    expect(linksContainer()?.className).toContain("hidden");

    await user.click(screen.getByRole("button", { name: "Previous Periods" }));

    expect(linksContainer()?.className).not.toContain("hidden");
    expect(screen.getByRole("link", { name: "Totals" })).toBeInTheDocument();
  });

  it("collapses the mobile submenu again after navigating to a link", async () => {
    const user = UserEvent.setup();
    renderSubmenuBar("/admin/activity");

    await user.click(screen.getByRole("button", { name: "Previous Periods" }));
    expect(linksContainer()?.className).not.toContain("hidden");

    await user.click(screen.getByRole("link", { name: "Totals" }));

    expect(linksContainer()?.className).toContain("hidden");
  });

  it("hides super-admin submenus from non-super users", () => {
    const { container } = renderSubmenuBar("/admin/locations", false);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows super-admin submenus for super users", () => {
    renderSubmenuBar("/admin/locations", true);
    expect(screen.getByRole("button", { name: "List" })).toBeInTheDocument();
  });
});

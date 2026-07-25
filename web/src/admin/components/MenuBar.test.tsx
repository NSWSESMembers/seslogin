import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi } from "vitest";
import UserEvent from "@testing-library/user-event";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router";
import MenuBar from "./MenuBar";

function renderMenuBar(initialPath = "/admin") {
  return render(
    <MemoryRouter initialEntries={[initialPath]}>
      <Routes>
        <Route
          path="*"
          element={<MenuBar onLogout={vi.fn()} isSuper={false} />}
        />
      </Routes>
    </MemoryRouter>,
  );
}

// jsdom doesn't load real CSS, so Tailwind's `hidden` class has no layout
// effect here — assert on the class toggle directly instead of visibility.
function linksContainer() {
  return document.getElementById("admin-menu-links")!;
}

describe("MenuBar", () => {
  it("shows the current section name on the mobile toggle", () => {
    renderMenuBar("/admin");
    expect(screen.getByRole("button", { name: "Home" })).toBeInTheDocument();
  });

  it("shows the current section name for a nested route", () => {
    renderMenuBar("/admin/activity/totals");
    expect(
      screen.getByRole("button", { name: "Activity" }),
    ).toBeInTheDocument();
  });

  it("hides the links behind the mobile toggle until opened", async () => {
    const user = UserEvent.setup();
    renderMenuBar("/admin");

    expect(linksContainer().className).toContain("hidden");

    await user.click(screen.getByRole("button", { name: "Home" }));

    expect(linksContainer().className).not.toContain("hidden");
    expect(screen.getByRole("link", { name: "Members" })).toBeInTheDocument();
  });

  it("collapses the mobile menu again after navigating to a link", async () => {
    const user = UserEvent.setup();
    renderMenuBar("/admin");

    await user.click(screen.getByRole("button", { name: "Home" }));
    expect(linksContainer().className).not.toContain("hidden");

    await user.click(screen.getByRole("link", { name: "Members" }));

    expect(linksContainer().className).toContain("hidden");
  });
});

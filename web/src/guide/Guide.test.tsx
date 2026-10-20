import "@testing-library/jest-dom/vitest";
import { describe, it, expect } from "vitest";
import UserEvent from "@testing-library/user-event";
import { render, screen, within } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router";
import guides from "virtual:guides";
import GuideApp from "./GuideApp";

// These render the real guides (web/guide/<guide>/*.md, compiled by
// guidePlugin.ts), so they also check that the guides build and that search
// finds real content.
function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path="/guide/*" element={<GuideApp />} />
      </Routes>
    </MemoryRouter>,
  );
}

describe("the guides", () => {
  it("has an admin and a kiosk guide, each starting with its quick start", () => {
    expect(Object.keys(guides)).toEqual(["admin", "kiosk"]);
    expect(guides.admin[0].slug).toBe("quick-start");
    expect(guides.kiosk[0].slug).toBe("quick-start");
  });

  it("offers both guides on the home page", () => {
    renderAt("/guide");
    expect(
      screen.getByRole("heading", { level: 1, name: "How can we help?" }),
    ).toBeInTheDocument();
    const main = screen.getByRole("main");
    expect(
      within(main).getByRole("link", { name: /Admin guide/ }),
    ).toHaveAttribute("href", "/guide/admin");
    expect(
      within(main).getByRole("link", { name: /Kiosk guide/ }),
    ).toHaveAttribute("href", "/guide/kiosk");
  });

  it("searches both guides from the home page", async () => {
    const user = UserEvent.setup();
    renderAt("/guide");
    await user.type(
      screen.getByLabelText("Search the guides", { selector: "#guide-search" }),
      "forgot to sign out",
    );
    const results = screen.getByRole("region", { name: "Search results" });
    const hrefs = within(results)
      .getAllByRole("link")
      .map((a) => a.getAttribute("href"));
    expect(hrefs.some((h) => h?.startsWith("/guide/admin/"))).toBe(true);
    expect(hrefs.some((h) => h?.startsWith("/guide/kiosk/"))).toBe(true);
  });

  it("says when nothing matches", async () => {
    const user = UserEvent.setup();
    renderAt("/guide");
    await user.type(
      screen.getByLabelText("Search the guides", { selector: "#guide-search" }),
      "zzzzzz",
    );
    expect(screen.getByRole("status")).toHaveTextContent(
      "Nothing in the guides matches",
    );
  });

  it.each(["admin", "kiosk"])(
    "lists every %s page on its contents page",
    (id) => {
      renderAt(`/guide/${id}`);
      const contents = screen.getByRole("navigation", { name: /contents$/ });
      for (const page of guides[id]) {
        expect(within(contents).getByText(page.title)).toBeInTheDocument();
      }
    },
  );

  it("searches within one guide", async () => {
    const user = UserEvent.setup();
    renderAt("/guide/admin");
    await user.type(
      screen.getByLabelText("Search the admin guide"),
      "kiosk expired",
    );
    const results = screen.getByRole("region", { name: "Search results" });
    expect(within(results).getAllByRole("link")[0]).toHaveAttribute(
      "href",
      "/guide/admin/kiosks#when-a-kiosk-expires",
    );
  });

  it("shows a page with its topics, contents and neighbours", () => {
    renderAt("/guide/admin/kiosks");
    expect(
      screen.getByRole("heading", { level: 1, name: "Setting up kiosks" }),
    ).toBeInTheDocument();
    const topics = screen.getAllByRole("navigation", {
      name: "Admin guide topics",
    })[0];
    expect(
      within(topics).getByRole("link", { name: "Setting up kiosks" }),
    ).toHaveAttribute("aria-current", "page");
    const onThisPage = screen.getAllByRole("navigation", {
      name: "On this page",
    })[0];
    expect(
      within(onThisPage).getByRole("link", { name: "Kiosk settings" }),
    ).toBeInTheDocument();
    const more = screen.getByRole("navigation", { name: "More of this guide" });
    expect(within(more).getByText(/Signing in/)).toBeInTheDocument();
    expect(within(more).getByText(/Members/)).toBeInTheDocument();
  });

  it("follows a link to another guide without reloading", async () => {
    const user = UserEvent.setup();
    renderAt("/guide/admin/kiosks");
    const prose = document.querySelector(".guide-prose")!;
    const toKiosk = [...prose.querySelectorAll("a")].find((a) =>
      a.getAttribute("href")?.startsWith("/guide/kiosk/"),
    )!;
    await user.click(toKiosk);
    expect(
      screen.getByRole("navigation", { name: "Breadcrumb" }),
    ).toHaveTextContent("Kiosk guide");
  });

  it("shows not found for an unknown guide or page", () => {
    renderAt("/guide/admin/no-such-page");
    expect(screen.queryByRole("article")).not.toBeInTheDocument();
    renderAt("/guide/nope");
    expect(
      screen.queryByRole("search", { name: /nope/ }),
    ).not.toBeInTheDocument();
  });

  it("puts a whole guide on its print page", () => {
    renderAt("/guide/kiosk/print");
    for (const page of guides.kiosk) {
      expect(
        screen.getByRole("heading", { level: 1, name: page.title }),
      ).toBeInTheDocument();
    }
  });
});

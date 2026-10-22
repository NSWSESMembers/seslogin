import { render, screen, within } from "@testing-library/react";
import ScanScreenCategories from "./ScanScreenCategories";
import { describe, expect, it, vitest } from "vitest";
import UserEvent from "@testing-library/user-event";

// The inlined category icon (see CategoryIcon) is decorative but, unlike the
// `<img>` it replaced, is real DOM content - some icons even carry glyphs of
// their own. Strip it so this reads the same button label text as before.
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
      "Flood Rescue: In Water",
      "Flood Rescue: Land Based",
      "Flood Rescue: On Water",
      "HCV: Logistics",
      "HCV: Operational",
      "Industrial & Domestic Rescue",
      "Job Ready",
      "Land Search",
      "Large Animal Rescue",
      "Map & Navigation",
      "Operate Comms. Equip.",
      "Other",
      "PIARO",
      "RCR",
      "Storm & Water: Ground",
      "Storm & Water: Heights",
      "Traffic Safety",
      "USAR",
      "VR",
    ]);
  });

  it("scrolls back to the top on drill-down and for a new transaction", async () => {
    const user = UserEvent.setup();
    const props = {
      screenPosition: "center" as const,
      onSelectCategory: vitest.fn(),
    };
    const { container, rerender } = render(
      <ScanScreenCategories uuid="a" {...props} />,
    );
    const screenDiv = container.firstElementChild as HTMLElement;

    screenDiv.scrollTop = 200;
    await user.click(screen.getByText("Training"));
    expect(screenDiv.scrollTop).toBe(0);

    screenDiv.scrollTop = 200;
    await user.click(screen.getAllByRole("button")[0]);
    expect(screenDiv.scrollTop).toBe(0);

    screenDiv.scrollTop = 200;
    rerender(<ScanScreenCategories uuid="b" {...props} />);
    expect(screenDiv.scrollTop).toBe(0);
  });
});

describe("ScanScreenCategories search", () => {
  function renderSearch(
    props: Partial<React.ComponentProps<typeof ScanScreenCategories>> = {},
  ) {
    const onSelect = vitest.fn();
    const user = UserEvent.setup();
    render(
      <ScanScreenCategories
        uuid="uuid-1"
        screenPosition="center"
        onSelectCategory={onSelect}
        categorySearch
        {...props}
      />,
    );
    return { onSelect, user };
  }

  function resultNames() {
    return within(screen.getByRole("list", { name: "Search results" }))
      .getAllByRole("button")
      .map((button) => button.getAttribute("aria-label"));
  }

  it("keeps the tiles until there are two letters", async () => {
    const { user } = renderSearch();
    await user.type(screen.getByRole("searchbox"), "s");
    expect(
      screen.getByRole("button", { name: "Training" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "Search results" })).toBeNull();

    await user.type(screen.getByRole("searchbox"), "t");
    expect(
      screen.getByRole("list", { name: "Search results" }),
    ).toBeInTheDocument();
    // The tiles are replaced by the results, not shown as well.
    expect(screen.queryByRole("button", { name: "Training" })).toBeNull();
  });

  it("shows the letters that were typed in bold", async () => {
    const { user } = renderSearch();
    await user.type(screen.getByRole("searchbox"), "stor");
    const first = within(screen.getByRole("list")).getAllByRole("button")[0];
    const bold = within(first).getByText("Stor");
    expect(bold.tagName).toBe("STRONG");
  });

  it("does nothing when Enter is pressed", async () => {
    const { user, onSelect } = renderSearch();
    await user.type(screen.getByRole("searchbox"), "stor{Enter}");
    expect(onSelect).not.toHaveBeenCalled();
    expect(resultNames()).toHaveLength(11);
  });

  it("says so when nothing matches", async () => {
    const { user } = renderSearch();
    await user.type(screen.getByRole("searchbox"), "zzz");
    const message = "No categories match \u201Czzz\u201D";
    // Drawn on screen, and announced to screen readers through the status line.
    expect(
      screen
        .getAllByText(message)
        .some((el) => el.tagName === "P" && !el.hasAttribute("role")),
    ).toBe(true);
    expect(screen.getByRole("status")).toHaveTextContent(message);
    expect(screen.queryByRole("list", { name: "Search results" })).toBeNull();
  });

  it("tells a screen reader how many matched, as each letter is typed", async () => {
    const { user } = renderSearch();
    expect(screen.getByRole("status")).toBeEmptyDOMElement();

    await user.type(screen.getByRole("searchbox"), "s");
    expect(screen.getByRole("status")).toBeEmptyDOMElement();

    await user.type(screen.getByRole("searchbox"), "tor");
    expect(screen.getByRole("status")).toHaveTextContent("11 categories found");

    await user.clear(screen.getByRole("searchbox"));
    await user.type(screen.getByRole("searchbox"), "comb stor");
    expect(screen.getByRole("status")).toHaveTextContent("1 category found");
  });

  it("goes back to the tiles when the search is cleared, and on a laptop typing carries on", async () => {
    const { user } = renderSearch({ autoFocusSearch: true });
    const box = screen.getByRole("searchbox");
    await user.type(box, "stor");
    await user.click(screen.getByRole("button", { name: "Clear search" }));
    expect(box).toHaveValue("");
    expect(box).toHaveFocus();
    expect(screen.getByText("Training")).toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "Search results" })).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Clear search" }),
    ).not.toBeInTheDocument();
    await user.keyboard("first");
    expect(box).toHaveValue("first");
  });

  it("leaves the cursor out of the box after Clear on a touch kiosk", async () => {
    const { user } = renderSearch({ autoFocusSearch: false });
    const box = screen.getByRole("searchbox");
    await user.type(box, "stor");
    await user.click(screen.getByRole("button", { name: "Clear search" }));
    expect(box).not.toHaveFocus();
  });

  describe("when a barcode scanner types into the box", () => {
    const hint =
      "Finish or cancel this sign-out before scanning another member.";

    it.each([
      ["", "10000002", false],
      ["", "1000000234", false],
      ["stor", "10000002", false],
      ["storm ", "123456789012", false],
      ["", "1000 0002", true],
    ])(
      "drops scanner ID %s/%s (paste: %s) without losing the query",
      async (query, id, paste) => {
        const { user, onSelect } = renderSearch({ autoFocusSearch: true });
        const box = screen.getByRole("searchbox");
        if (query) await user.keyboard(query);
        if (paste) await user.paste(id);
        else await user.keyboard(id + "{Enter}");
        expect(box).toHaveValue(query);
        expect(screen.getByRole("status")).toHaveTextContent(hint);
        expect(
          screen.getAllByText(hint).some((el) => !el.hasAttribute("role")),
        ).toBe(true);
        expect(onSelect).not.toHaveBeenCalled();
        await user.keyboard("gr");
        expect(box).toHaveValue(query + "gr");
        expect(screen.queryByText(hint)).toBeNull();
      },
    );

    it("lets the next word be typed once the ID is gone", async () => {
      const { user } = renderSearch({ autoFocusSearch: true });
      await user.keyboard("storm12345678{Enter} ground");
      expect(screen.getByRole("searchbox")).toHaveValue("storm ground");
    });

    it("takes an ID out of the middle of a search, keeping the rest", async () => {
      const { user } = renderSearch({ autoFocusSearch: true });
      const box = screen.getByRole("searchbox");
      await user.type(box, "storm");
      await user.type(box, "10000002", {
        initialSelectionStart: 2,
        initialSelectionEnd: 2,
      });
      expect(box).toHaveValue("storm");
      expect(screen.getByRole("status")).toHaveTextContent(hint);
    });

    it("still searches for a few digits", async () => {
      const { user } = renderSearch();
      await user.type(screen.getByRole("searchbox"), "1234567");
      expect(screen.queryByText(hint)).toBeNull();
      expect(screen.getByRole("searchbox")).toHaveValue("1234567");
    });
  });

  describe("inside a role", () => {
    async function intoTraining() {
      const rendered = renderSearch();
      await rendered.user.click(screen.getByText("Training"));
      return rendered;
    }

    it("shows no role switch at the top level, even with results", async () => {
      const { user } = renderSearch();
      await user.type(screen.getByRole("searchbox"), "stor");
      expect(screen.queryByRole("group", { name: "Search in" })).toBeNull();
    });

    it("shows the switch only with results, starting on this role, and searches only that role", async () => {
      const { user } = await intoTraining();
      expect(screen.queryByRole("group", { name: "Search in" })).toBeNull();
      await user.type(screen.getByRole("searchbox"), "stor");
      const group = screen.getByRole("group", { name: "Search in" });
      expect(
        within(group).getByRole("button", { name: "This role" }),
      ).toHaveAttribute("aria-pressed", "true");
      expect(
        within(group).getByRole("button", { name: "All categories" }),
      ).toHaveAttribute("aria-pressed", "false");

      expect(resultNames()).toEqual([
        "Training, Storm & Water: Ground",
        "Training, Storm & Water: Heights",
      ]);
    });

    it("searches every category on All, this role again on This role, and keeps what was typed across Back", async () => {
      const { user } = await intoTraining();
      const box = screen.getByRole("searchbox");
      await user.type(box, "stor");
      await user.click(screen.getByRole("button", { name: "All categories" }));
      expect(resultNames()).toHaveLength(11);
      await user.click(screen.getByRole("button", { name: "This role" }));
      expect(resultNames()).toHaveLength(2);
      await user.click(screen.getByRole("button", { name: /Categories/ }));
      expect(box).toHaveValue("stor");
      expect(resultNames()).toHaveLength(11);
    });

    it("returns to the tiles of that role when the box is emptied", async () => {
      const { user } = await intoTraining();
      await user.type(screen.getByRole("searchbox"), "stor");
      await user.click(screen.getByRole("button", { name: "Clear search" }));
      expect(screen.getByText("AIIMS")).toBeInTheDocument();
      expect(screen.getByText("Training")).toBeInTheDocument();
    });

    it("hides the switch when the search is cleared, and starts on this role again", async () => {
      const { user } = await intoTraining();
      const box = screen.getByRole("searchbox");
      await user.type(box, "stor");
      await user.click(screen.getByRole("button", { name: "All categories" }));
      await user.click(screen.getByRole("button", { name: "Clear search" }));
      expect(screen.queryByRole("group", { name: "Search in" })).toBeNull();
      await user.type(box, "stor");
      expect(screen.getByRole("button", { name: "This role" })).toHaveAttribute(
        "aria-pressed",
        "true",
      );
    });
  });

  describe("the keyboard", () => {
    it("leaves the cursor alone while the screen is parked to the side", () => {
      renderSearch({ autoFocusSearch: true, screenPosition: "offLeft" });
      expect(screen.getByRole("searchbox")).not.toHaveFocus();
    });
  });
});

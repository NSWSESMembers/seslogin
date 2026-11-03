import { render, screen } from "@testing-library/react";
import UserEvent from "@testing-library/user-event";
import { describe, expect, it, vitest } from "vitest";
import ScanScreenQuickPick from "./ScanScreenQuickPick";

// "Training > AIIMS", which the quick pick offers and the search can find.
const AIIMS_ID = "RX2bfpU6ppvV";
const suggestions = {
  location: [{ categoryId: AIIMS_ID, peopleNames: ["Jane"] }],
  person: [],
};

describe("ScanScreenQuickPick search", () => {
  function renderQuickPick(
    props: Partial<React.ComponentProps<typeof ScanScreenQuickPick>> = {},
  ) {
    const onSelectCategory = vitest.fn();
    render(
      <ScanScreenQuickPick
        screenPosition="center"
        uuid="uuid-1"
        suggestions={suggestions}
        onSelectCategory={onSelectCategory}
        onSkip={vitest.fn()}
        categorySearch
        {...props}
      />,
    );
    return { onSelectCategory, user: UserEvent.setup() };
  }

  it("has no search box unless it is switched on", () => {
    renderQuickPick({ categorySearch: false });
    expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
    expect(screen.getByText("This location")).toBeInTheDocument();
  });

  it("swaps the quick picks for results, chooses a result, and brings them back when cleared", async () => {
    const { user, onSelectCategory } = renderQuickPick();
    await user.type(screen.getByRole("searchbox"), "aiims");

    expect(screen.queryByText("This location")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "More categories" }),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Training, AIIMS" }));
    expect(onSelectCategory).toHaveBeenCalledWith("uuid-1", AIIMS_ID);

    await user.click(screen.getByRole("button", { name: "Clear search" }));
    expect(screen.getByText("This location")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "More categories" }),
    ).toBeInTheDocument();
  });

  it("puts the cursor in the box on a laptop, not on a touch kiosk", () => {
    const { unmount } = render(
      <ScanScreenQuickPick
        screenPosition="center"
        uuid="u"
        suggestions={suggestions}
        onSelectCategory={vitest.fn()}
        onSkip={vitest.fn()}
        categorySearch
        autoFocusSearch
      />,
    );
    expect(screen.getByRole("searchbox")).toHaveFocus();
    unmount();

    renderQuickPick({ autoFocusSearch: false });
    expect(screen.getByRole("searchbox")).not.toHaveFocus();
  });

  it("leaves the cursor out of the box after Clear on a touch kiosk", async () => {
    const { user } = renderQuickPick({ autoFocusSearch: false });
    const box = screen.getByRole("searchbox");
    await user.type(box, "aiims");
    await user.click(screen.getByRole("button", { name: "Clear search" }));
    expect(box).not.toHaveFocus();
  });
});

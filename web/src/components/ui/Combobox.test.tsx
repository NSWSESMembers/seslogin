import { beforeEach, describe, expect, test, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import UserEvent from "@testing-library/user-event";
import Combobox from "./Combobox";
import type { ComboboxOption, ComboboxProps } from "./Combobox";

const BOAT = "Assessor - Flood Operator L2 (Boat)";
const TRAINER = "Trainer - Field Core Skills";
const NAVIGATION = "Workshop - Participant - Navigation";
const STORM = "Workshop - Participant - Storm Damage";
const MEETING = "Meeting";
const RETIRED = "Retired Activity";

const CATEGORIES: ComboboxOption[] = [
  { value: "c1", label: BOAT },
  { value: "c2", label: TRAINER },
  { value: "c3", label: NAVIGATION },
  { value: "c4", label: STORM },
  { value: "c5", label: MEETING },
  { value: "c6", label: RETIRED, disabled: true },
];

/** jsdom's default. Above the `md` breakpoint, so the combobox is the default branch. */
const DESKTOP_WIDTH = 1024;

function renderCombobox({
  onSubmit,
  ...props
}: Partial<ComboboxProps> & { onSubmit?: (data: FormData) => void } = {}) {
  const result = render(
    <form action={onSubmit}>
      <Combobox
        id="category"
        name="category"
        options={CATEGORIES}
        placeholder="-- Select category --"
        emptyText="No categories match"
        {...props}
      />
      <button type="submit">Save</button>
    </form>,
  );
  return { ...result, user: UserEvent.setup() };
}

function input() {
  return screen.getByRole("combobox");
}

function optionLabels() {
  return screen.queryAllByRole("option").map((o) => o.textContent);
}

beforeEach(() => {
  window.innerWidth = DESKTOP_WIDTH;
});

describe("opening and closing", () => {
  test("starts closed, with no listbox", () => {
    renderCombobox();
    expect(input()).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  test("clicking opens it and shows every option", async () => {
    const { user } = renderCombobox();
    await user.click(input());
    expect(input()).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("listbox")).toBeInTheDocument();
    expect(optionLabels()).toHaveLength(CATEGORIES.length);
  });

  test("clicking again closes it", async () => {
    const { user } = renderCombobox();
    await user.click(input());
    await user.click(input());
    expect(input()).toHaveAttribute("aria-expanded", "false");
  });

  test("focusing it with the keyboard does not open it", async () => {
    const { user } = renderCombobox();
    await user.tab();
    expect(input()).toHaveFocus();
    expect(input()).toHaveAttribute("aria-expanded", "false");
  });

  test("ArrowDown opens it with the first option active", async () => {
    const { user } = renderCombobox();
    await user.tab();
    await user.keyboard("{ArrowDown}");
    expect(input()).toHaveAttribute("aria-expanded", "true");
    expect(input()).toHaveAttribute("aria-activedescendant", "category-opt-0");
  });

  test("Escape closes it and keeps the selection", async () => {
    const { user } = renderCombobox({ defaultValue: "c2" });
    await user.click(input());
    await user.keyboard("{Escape}");
    expect(input()).toHaveAttribute("aria-expanded", "false");
    expect(input()).toHaveValue(TRAINER);
  });

  test("clicking outside closes it", async () => {
    const { user } = renderCombobox();
    await user.click(input());
    await user.click(document.body);
    expect(input()).toHaveAttribute("aria-expanded", "false");
  });
});

describe("filtering", () => {
  test("typing narrows the list", async () => {
    const { user } = renderCombobox();
    await user.click(input());
    await user.type(input(), "nav");
    expect(optionLabels()).toEqual([NAVIGATION]);
    expect(screen.queryByText(STORM)).not.toBeInTheDocument();
  });

  test("typing while closed starts a fresh search over the whole list", async () => {
    const { user } = renderCombobox({ defaultValue: "c4" });
    await user.tab();
    await user.keyboard("nav");
    expect(input()).toHaveValue("nav");
    expect(optionLabels()).toEqual([NAVIGATION]);
  });

  test("a query matching nothing says so and offers no options", async () => {
    const { user } = renderCombobox();
    await user.click(input());
    await user.type(input(), "zzz");
    expect(screen.queryAllByRole("option")).toEqual([]);
    expect(screen.getByText("No categories match “zzz”.")).toBeInTheDocument();
  });

  test("clearing the query restores the full list", async () => {
    const { user } = renderCombobox();
    await user.click(input());
    await user.type(input(), "nav");
    await user.clear(input());
    expect(optionLabels()).toHaveLength(CATEGORIES.length);
  });
});

describe("keyboard navigation", () => {
  test("arrows move the active option and keep focus on the input", async () => {
    const { user } = renderCombobox();
    await user.click(input());
    await user.keyboard("{ArrowDown}{ArrowDown}");
    expect(input()).toHaveAttribute("aria-activedescendant", "category-opt-2");
    expect(input()).toHaveFocus();
    await user.keyboard("{ArrowUp}");
    expect(input()).toHaveAttribute("aria-activedescendant", "category-opt-1");
  });

  test("does not wrap at either end", async () => {
    const { user } = renderCombobox();
    await user.click(input());
    await user.keyboard("{ArrowUp}{ArrowUp}");
    expect(input()).toHaveAttribute("aria-activedescendant", "category-opt-0");
    await user.keyboard("{ArrowDown>7/}");
    expect(input()).toHaveAttribute(
      "aria-activedescendant",
      `category-opt-${CATEGORIES.length - 1}`,
    );
  });

  test("Enter commits the active option", async () => {
    const onChange = vi.fn();
    const { user } = renderCombobox({ onChange });
    await user.click(input());
    await user.keyboard("{ArrowDown}{Enter}");
    expect(onChange).toHaveBeenCalledWith("c2", CATEGORIES[1]);
    expect(input()).toHaveValue(TRAINER);
    expect(input()).toHaveAttribute("aria-expanded", "false");
  });

  test("Enter commits the sole match without arrowing to it first", async () => {
    const onChange = vi.fn();
    const { user } = renderCombobox({ onChange });
    await user.click(input());
    await user.type(input(), "boat");
    // Typing resets the active row to the top of the narrowed list, so the one
    // remaining match is already active and needs no arrow key.
    await user.keyboard("{Enter}");
    expect(onChange).toHaveBeenCalledWith("c1", CATEGORIES[0]);
  });

  test("Enter with nothing matching dismisses the list and selects nothing", async () => {
    const onChange = vi.fn();
    const { user } = renderCombobox({ onChange });
    await user.click(input());
    await user.type(input(), "zzz");
    await user.keyboard("{Enter}");
    expect(onChange).not.toHaveBeenCalled();
    expect(input()).toHaveAttribute("aria-expanded", "false");
  });

  test("Enter while closed submits the form instead of being swallowed", async () => {
    const onSubmit = vi.fn();
    const { user } = renderCombobox({ onSubmit, defaultValue: "c2" });
    await user.tab();
    await user.keyboard("{Enter}");
    expect(onSubmit).toHaveBeenCalledOnce();
  });

  test("Tab closes without committing the active option", async () => {
    const onChange = vi.fn();
    const { user } = renderCombobox({ onChange });
    await user.click(input());
    await user.keyboard("{ArrowDown}");
    await user.tab();
    expect(onChange).not.toHaveBeenCalled();
    expect(input()).toHaveAttribute("aria-expanded", "false");
  });

  test("closing restores the selected label, discarding the query", async () => {
    const { user } = renderCombobox({ defaultValue: "c5" });
    await user.click(input());
    await user.type(input(), "nav");
    await user.keyboard("{Escape}");
    expect(input()).toHaveValue(MEETING);
  });
});

describe("selecting with a pointer", () => {
  test("clicking an option commits it and closes the list", async () => {
    const onChange = vi.fn();
    const { user } = renderCombobox({ onChange });
    await user.click(input());
    await user.click(screen.getByRole("option", { name: NAVIGATION }));
    expect(onChange).toHaveBeenCalledWith("c3", CATEGORIES[2]);
    expect(input()).toHaveValue(NAVIGATION);
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  test("exactly the selected option is marked selected when reopened", async () => {
    const { user } = renderCombobox({ defaultValue: "c3" });
    await user.click(input());
    const selected = screen
      .getAllByRole("option")
      .filter((o) => o.getAttribute("aria-selected") === "true");
    expect(selected.map((o) => o.textContent)).toEqual([NAVIGATION]);
  });

  test("a disabled option cannot be chosen", async () => {
    const onChange = vi.fn();
    const { user } = renderCombobox({ onChange });
    await user.click(input());
    const retired = screen.getByRole("option", { name: RETIRED });
    expect(retired).toHaveAttribute("aria-disabled", "true");
    await user.click(retired);
    expect(onChange).not.toHaveBeenCalled();
    expect(input()).toHaveAttribute("aria-expanded", "true");
  });
});

describe("controlled and uncontrolled", () => {
  test("defaultValue shows that option's label on first render", () => {
    renderCombobox({ defaultValue: "c2" });
    expect(input()).toHaveValue(TRAINER);
  });

  test("a controlled value does not move until the parent says so", async () => {
    const onChange = vi.fn();
    const { user } = renderCombobox({ value: "c5", onChange });
    await user.click(input());
    await user.click(screen.getByRole("option", { name: NAVIGATION }));
    expect(onChange).toHaveBeenCalledWith("c3", CATEGORIES[2]);
    expect(input()).toHaveValue(MEETING);
  });

  test("a value with no matching option is surfaced, not hidden", () => {
    renderCombobox({ value: "deleted-id" });
    expect(input()).toHaveValue("deleted-id");
    expect(input()).toHaveAttribute("aria-invalid", "true");
    expect(
      screen.getByText("This option is no longer available."),
    ).toBeInTheDocument();
  });
});

describe("FormData", () => {
  test("submits the selected option's value under `name`", async () => {
    const onSubmit = vi.fn();
    const { user } = renderCombobox({ onSubmit });
    await user.click(input());
    await user.click(screen.getByRole("option", { name: NAVIGATION }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit.mock.calls[0][0].get("category")).toBe("c3");
  });

  test("submits an untouched defaultValue", async () => {
    const onSubmit = vi.fn();
    const { user } = renderCombobox({ onSubmit, defaultValue: "c4" });
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit.mock.calls[0][0].get("category")).toBe("c4");
  });

  test("contributes nothing when `name` is omitted", async () => {
    const onSubmit = vi.fn();
    const { user } = renderCombobox({
      onSubmit,
      name: undefined,
      defaultValue: "c4",
    });
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit.mock.calls[0][0].get("category")).toBeNull();
  });
});

describe("required", () => {
  test("blocks submission when nothing is selected", async () => {
    const onSubmit = vi.fn();
    const { user } = renderCombobox({ onSubmit, required: true });
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit).not.toHaveBeenCalled();
  });

  test("blocks submission when a query was typed but nothing chosen", async () => {
    const onSubmit = vi.fn();
    const { user } = renderCombobox({ onSubmit, required: true });
    await user.click(input());
    await user.type(input(), "nav");
    // Abandoning a query must leave no phantom selection behind: the box empties
    // back out and the field is still empty as far as the form is concerned.
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit).not.toHaveBeenCalled();
    expect(input()).toHaveValue("");
  });

  test("blocks submission of a value whose option no longer exists", async () => {
    const onSubmit = vi.fn();
    const { user } = renderCombobox({ onSubmit, value: "deleted-id" });
    // The box is non-empty here, so native `required` would not catch it even
    // if it were set — this is the custom-validity path.
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit).not.toHaveBeenCalled();
  });

  test("allows submission once an option is chosen", async () => {
    const onSubmit = vi.fn();
    const { user } = renderCombobox({ onSubmit, required: true });
    await user.click(input());
    await user.type(input(), "nav");
    await user.keyboard("{Enter}");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit.mock.calls[0][0].get("category")).toBe("c3");
  });
});

describe("clearing", () => {
  test("the clear button empties the selection", async () => {
    const onChange = vi.fn();
    const { user } = renderCombobox({ defaultValue: "c2", onChange });
    await user.click(screen.getByRole("button", { name: "Clear selection" }));
    expect(onChange).toHaveBeenCalledWith("", null);
    expect(input()).toHaveValue("");
  });

  test("Backspace on an empty query clears the selection", async () => {
    const onChange = vi.fn();
    const { user } = renderCombobox({ defaultValue: "c2", onChange });
    await user.tab();
    await user.keyboard("{Backspace}");
    expect(onChange).toHaveBeenCalledWith("", null);
  });

  test("a required field offers no clear button", () => {
    renderCombobox({ defaultValue: "c2", required: true });
    expect(
      screen.queryByRole("button", { name: "Clear selection" }),
    ).not.toBeInTheDocument();
  });
});

test("resetting the form restores the default value", async () => {
  const { user, container } = renderCombobox({ defaultValue: "c2" });
  await user.click(input());
  await user.click(screen.getByRole("option", { name: NAVIGATION }));
  expect(input()).toHaveValue(NAVIGATION);

  fireEvent.reset(container.querySelector("form")!);
  expect(input()).toHaveValue(TRAINER);
});

describe("nativeOnSmallScreens", () => {
  test("renders a native select below the md breakpoint", async () => {
    window.innerWidth = 375;
    const onSubmit = vi.fn();
    const { user } = renderCombobox({
      onSubmit,
      nativeOnSmallScreens: true,
      defaultValue: "c3",
    });

    const control = input();
    expect(control.tagName).toBe("SELECT");
    expect(control).toHaveValue("c3");

    await user.selectOptions(control, "c4");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit.mock.calls[0][0].get("category")).toBe("c4");
  });

  test("renders the combobox at md and above", () => {
    renderCombobox({ nativeOnSmallScreens: true });
    expect(input().tagName).toBe("INPUT");
  });
});

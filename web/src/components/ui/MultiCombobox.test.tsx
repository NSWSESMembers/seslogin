import { beforeEach, describe, expect, test, vi } from "vitest";
import { createEvent, fireEvent, render, screen } from "@testing-library/react";
import UserEvent from "@testing-library/user-event";
import MultiCombobox from "./MultiCombobox";
import type { ComboboxOption, MultiComboboxProps } from "./MultiCombobox";

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

/** jsdom's default. Above the `md` breakpoint isn't load-bearing here — unlike
 * `Combobox`, this control has no `nativeOnSmallScreens` branch — but kept for
 * parity with `Combobox.test.tsx`. */
const DESKTOP_WIDTH = 1024;

function renderMultiCombobox({
  onSubmit,
  ...props
}: Partial<MultiComboboxProps> & {
  onSubmit?: (data: FormData) => void;
} = {}) {
  const result = render(
    <form action={onSubmit}>
      <MultiCombobox
        id="categories"
        name="categories"
        options={CATEGORIES}
        placeholder="-- Select categories --"
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

function pillLabels() {
  return screen
    .queryAllByRole("button", { name: /^Remove / })
    .map((b) => b.getAttribute("aria-label")!.replace(/^Remove /, ""));
}

beforeEach(() => {
  window.innerWidth = DESKTOP_WIDTH;
});

describe("opening and closing", () => {
  test("starts closed, with no listbox", () => {
    renderMultiCombobox();
    expect(input()).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  test("clicking opens it and shows every option", async () => {
    const { user } = renderMultiCombobox();
    await user.click(input());
    expect(input()).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("listbox")).toBeInTheDocument();
    expect(optionLabels()).toHaveLength(CATEGORIES.length);
  });

  test("clicking again closes it", async () => {
    const { user } = renderMultiCombobox();
    await user.click(input());
    await user.click(input());
    expect(input()).toHaveAttribute("aria-expanded", "false");
  });

  test("focusing it with the keyboard does not open it", async () => {
    const { user } = renderMultiCombobox();
    await user.tab();
    expect(input()).toHaveFocus();
    expect(input()).toHaveAttribute("aria-expanded", "false");
  });

  test("ArrowDown opens it with the first option active", async () => {
    const { user } = renderMultiCombobox();
    await user.tab();
    await user.keyboard("{ArrowDown}");
    expect(input()).toHaveAttribute("aria-expanded", "true");
    expect(input()).toHaveAttribute(
      "aria-activedescendant",
      "categories-opt-0",
    );
  });

  test("Escape closes it without discarding the selection", async () => {
    const { user } = renderMultiCombobox({ defaultValue: ["c2"] });
    await user.click(input());
    await user.keyboard("{Escape}");
    expect(input()).toHaveAttribute("aria-expanded", "false");
    expect(pillLabels()).toEqual([TRAINER]);
  });

  test("Escape while closed is not swallowed", () => {
    renderMultiCombobox();
    const event = createEvent.keyDown(input(), { key: "Escape" });
    fireEvent(input(), event);
    expect(event.defaultPrevented).toBe(false);
  });

  test("clicking outside closes it", async () => {
    const { user } = renderMultiCombobox();
    await user.click(input());
    await user.click(document.body);
    expect(input()).toHaveAttribute("aria-expanded", "false");
  });
});

describe("filtering", () => {
  test("typing narrows the list", async () => {
    const { user } = renderMultiCombobox();
    await user.click(input());
    await user.type(input(), "nav");
    expect(optionLabels()).toEqual([NAVIGATION]);
    expect(screen.queryByText(STORM)).not.toBeInTheDocument();
  });

  test("a query matching nothing says so and offers no options", async () => {
    const { user } = renderMultiCombobox();
    await user.click(input());
    await user.type(input(), "zzz");
    expect(screen.queryAllByRole("option")).toEqual([]);
    expect(screen.getByText("No categories match “zzz”.")).toBeInTheDocument();
  });

  test("clearing the query restores the full list", async () => {
    const { user } = renderMultiCombobox();
    await user.click(input());
    await user.type(input(), "nav");
    await user.clear(input());
    expect(optionLabels()).toHaveLength(CATEGORIES.length);
  });
});

describe("selecting", () => {
  test("clicking an option adds a pill, clears the query, and keeps the list open", async () => {
    const { user } = renderMultiCombobox();
    await user.click(input());
    await user.type(input(), "nav");
    await user.click(screen.getByRole("option", { name: NAVIGATION }));
    expect(pillLabels()).toEqual([NAVIGATION]);
    expect(input()).toHaveValue("");
    expect(input()).toHaveAttribute("aria-expanded", "true");
  });

  test("clicking a selected option removes its pill", async () => {
    const { user } = renderMultiCombobox({ defaultValue: ["c3"] });
    await user.click(input());
    await user.click(screen.getByRole("option", { name: NAVIGATION }));
    expect(pillLabels()).toEqual([]);
  });

  test("aria-selected tracks exactly the chosen rows", async () => {
    const { user } = renderMultiCombobox({ defaultValue: ["c2", "c4"] });
    await user.click(input());
    const selected = screen
      .getAllByRole("option")
      .filter((o) => o.getAttribute("aria-selected") === "true");
    // Selected rows carry a leading ✓ marker (see `comboboxOptionCheck`).
    expect(selected.map((o) => o.textContent)).toEqual(
      expect.arrayContaining([`✓${TRAINER}`, `✓${STORM}`]),
    );
    expect(selected).toHaveLength(2);
  });

  test("the listbox is aria-multiselectable", async () => {
    const { user } = renderMultiCombobox();
    await user.click(input());
    expect(screen.getByRole("listbox")).toHaveAttribute(
      "aria-multiselectable",
      "true",
    );
  });

  test("a disabled option cannot be added", async () => {
    const { user } = renderMultiCombobox();
    await user.click(input());
    const retired = screen.getByRole("option", { name: RETIRED });
    expect(retired).toHaveAttribute("aria-disabled", "true");
    await user.click(retired);
    expect(pillLabels()).toEqual([]);
    expect(input()).toHaveAttribute("aria-expanded", "true");
  });

  test("the active row stays on the option just toggled", async () => {
    const { user } = renderMultiCombobox();
    await user.click(input());
    await user.type(input(), "nav");
    await user.click(screen.getByRole("option", { name: NAVIGATION }));
    // Query is cleared, so the full list is showing again; the row for what
    // was just added (index 2, NAVIGATION) should be the active one, not 0.
    expect(input()).toHaveAttribute(
      "aria-activedescendant",
      "categories-opt-2",
    );
  });
});

describe("keyboard", () => {
  test("arrows move the active option and keep focus on the input", async () => {
    const { user } = renderMultiCombobox();
    await user.click(input());
    await user.keyboard("{ArrowDown}{ArrowDown}");
    expect(input()).toHaveAttribute(
      "aria-activedescendant",
      "categories-opt-2",
    );
    expect(input()).toHaveFocus();
  });

  test("does not wrap at either end", async () => {
    const { user } = renderMultiCombobox();
    await user.click(input());
    await user.keyboard("{ArrowUp}{ArrowUp}");
    expect(input()).toHaveAttribute(
      "aria-activedescendant",
      "categories-opt-0",
    );
    await user.keyboard("{ArrowDown>7/}");
    expect(input()).toHaveAttribute(
      "aria-activedescendant",
      `categories-opt-${CATEGORIES.length - 1}`,
    );
  });

  test("Enter toggles the active option and leaves the list open", async () => {
    const onChange = vi.fn();
    const { user } = renderMultiCombobox({ onChange });
    await user.click(input());
    // Clicking opens with the active row at index 0; ArrowDown then moves it
    // to index 1 (TRAINER) before Enter toggles it.
    await user.keyboard("{ArrowDown}{Enter}");
    expect(onChange).toHaveBeenCalledWith(["c2"], [CATEGORIES[1]]);
    expect(pillLabels()).toEqual([TRAINER]);
    expect(input()).toHaveAttribute("aria-expanded", "true");
  });

  test("Enter with nothing matching dismisses the list and adds nothing", async () => {
    const onChange = vi.fn();
    const { user } = renderMultiCombobox({ onChange });
    await user.click(input());
    await user.type(input(), "zzz");
    await user.keyboard("{Enter}");
    expect(onChange).not.toHaveBeenCalled();
    expect(input()).toHaveAttribute("aria-expanded", "false");
  });

  test("Enter while closed submits the form instead of being swallowed", async () => {
    const onSubmit = vi.fn();
    const { user } = renderMultiCombobox({ onSubmit, defaultValue: ["c2"] });
    // Focused directly rather than via Tab: a pill's own × button is a real
    // tab stop before the input (see "removing"), so Tab from nothing would
    // land there first, not on the combobox this test means to exercise.
    input().focus();
    await user.keyboard("{Enter}");
    expect(onSubmit).toHaveBeenCalledOnce();
  });

  test("Tab closes without toggling the active option", async () => {
    const onChange = vi.fn();
    const { user } = renderMultiCombobox({ onChange });
    await user.click(input());
    await user.keyboard("{ArrowDown}");
    await user.tab();
    expect(onChange).not.toHaveBeenCalled();
    expect(input()).toHaveAttribute("aria-expanded", "false");
  });

  test("Backspace on an empty query removes the last pill", async () => {
    const onChange = vi.fn();
    const { user } = renderMultiCombobox({
      defaultValue: ["c2", "c4"],
      onChange,
    });
    // Focused directly: a pill's × is an earlier tab stop than the input.
    input().focus();
    await user.keyboard("{Backspace}");
    expect(onChange).toHaveBeenCalledWith(["c2"], [CATEGORIES[1]]);
    expect(pillLabels()).toEqual([TRAINER]);
  });

  test("Delete on an empty query also removes the last pill", async () => {
    const { user } = renderMultiCombobox({ defaultValue: ["c2", "c4"] });
    input().focus();
    await user.keyboard("{Delete}");
    expect(pillLabels()).toEqual([TRAINER]);
  });

  test("Backspace with a query typed edits the text instead of removing a pill", async () => {
    const { user } = renderMultiCombobox({ defaultValue: ["c2"] });
    await user.click(input());
    await user.type(input(), "nav");
    await user.keyboard("{Backspace}");
    expect(input()).toHaveValue("na");
    expect(pillLabels()).toEqual([TRAINER]);
  });

  test("Delete with a query typed edits the text instead of removing a pill", async () => {
    const { user } = renderMultiCombobox({ defaultValue: ["c2"] });
    await user.click(input());
    await user.type(input(), "nav");
    await user.keyboard("{ArrowLeft}{Delete}");
    expect(input()).toHaveValue("na");
    expect(pillLabels()).toEqual([TRAINER]);
  });

  test("Backspace with nothing selected does nothing", async () => {
    const onChange = vi.fn();
    const { user } = renderMultiCombobox({ onChange });
    await user.tab();
    await user.keyboard("{Backspace}");
    expect(onChange).not.toHaveBeenCalled();
  });
});

describe("removing", () => {
  test("a pill's × removes just that pill", async () => {
    const onChange = vi.fn();
    const { user } = renderMultiCombobox({
      defaultValue: ["c2", "c4"],
      onChange,
    });
    await user.click(screen.getByRole("button", { name: `Remove ${TRAINER}` }));
    expect(onChange).toHaveBeenCalledWith(["c4"], [CATEGORIES[3]]);
    expect(pillLabels()).toEqual([STORM]);
  });

  test("Clear all empties every pill", async () => {
    const onChange = vi.fn();
    const { user } = renderMultiCombobox({
      defaultValue: ["c2", "c4"],
      onChange,
    });
    await user.click(screen.getByRole("button", { name: "Clear all" }));
    expect(onChange).toHaveBeenCalledWith([], []);
    expect(pillLabels()).toEqual([]);
  });

  test("Clear all is absent when required", () => {
    renderMultiCombobox({ defaultValue: ["c2"], required: true });
    expect(
      screen.queryByRole("button", { name: "Clear all" }),
    ).not.toBeInTheDocument();
  });

  test("Clear all is absent when nothing is selected", () => {
    renderMultiCombobox();
    expect(
      screen.queryByRole("button", { name: "Clear all" }),
    ).not.toBeInTheDocument();
  });
});

describe("FormData", () => {
  test("submits one entry per selection under `name`, in pill order", async () => {
    const onSubmit = vi.fn();
    const { user } = renderMultiCombobox({ onSubmit });
    await user.click(input());
    await user.click(screen.getByRole("option", { name: NAVIGATION }));
    // Still open — picking one option doesn't close the list — so the next
    // pick doesn't need to reopen it.
    await user.click(screen.getByRole("option", { name: MEETING }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit.mock.calls[0][0].getAll("categories")).toEqual([
      "c3",
      "c5",
    ]);
  });

  test("submits an untouched defaultValue", async () => {
    const onSubmit = vi.fn();
    const { user } = renderMultiCombobox({
      onSubmit,
      defaultValue: ["c2", "c4"],
    });
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit.mock.calls[0][0].getAll("categories")).toEqual([
      "c2",
      "c4",
    ]);
  });

  test("contributes nothing when `name` is omitted", async () => {
    const onSubmit = vi.fn();
    const { user } = renderMultiCombobox({
      onSubmit,
      name: undefined,
      defaultValue: ["c2"],
    });
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit.mock.calls[0][0].getAll("categories")).toEqual([]);
  });

  test("a defaultValue id with no matching option renders no pill and submits nothing for it", async () => {
    const onSubmit = vi.fn();
    const { user } = renderMultiCombobox({
      onSubmit,
      defaultValue: ["c2", "deleted-id"],
    });
    expect(pillLabels()).toEqual([TRAINER]);
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit.mock.calls[0][0].getAll("categories")).toEqual(["c2"]);
  });
});

describe("required", () => {
  test("blocks submission when nothing is selected", async () => {
    const onSubmit = vi.fn();
    const { user } = renderMultiCombobox({ onSubmit, required: true });
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit).not.toHaveBeenCalled();
  });

  test("blocks submission when a query was typed but nothing chosen", async () => {
    const onSubmit = vi.fn();
    const { user } = renderMultiCombobox({ onSubmit, required: true });
    await user.click(input());
    await user.type(input(), "nav");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit).not.toHaveBeenCalled();
  });

  test("allows submission once one option is chosen", async () => {
    const onSubmit = vi.fn();
    const { user } = renderMultiCombobox({ onSubmit, required: true });
    await user.click(input());
    await user.click(screen.getByRole("option", { name: NAVIGATION }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit.mock.calls[0][0].getAll("categories")).toEqual(["c3"]);
  });

  test("a stale defaultValue id alone still blocks submission", async () => {
    const onSubmit = vi.fn();
    const { user } = renderMultiCombobox({
      onSubmit,
      required: true,
      defaultValue: ["deleted-id"],
    });
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit).not.toHaveBeenCalled();
  });
});

test("resetting the form restores the default value", async () => {
  const { user, container } = renderMultiCombobox({ defaultValue: ["c2"] });
  await user.click(input());
  await user.click(screen.getByRole("option", { name: NAVIGATION }));
  expect(pillLabels()).toEqual(expect.arrayContaining([TRAINER, NAVIGATION]));

  fireEvent.reset(container.querySelector("form")!);
  expect(pillLabels()).toEqual([TRAINER]);
});

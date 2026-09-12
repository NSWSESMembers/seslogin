import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi } from "vitest";
import UserEvent from "@testing-library/user-event";
import { render, screen } from "@testing-library/react";
import { useState } from "react";
import MultiSelectList, { type MultiSelectOption } from "./MultiSelectList";

const LOCATIONS: MultiSelectOption[] = [
  { id: "1", name: "Penrith North" },
  { id: "2", name: "Penrith South" },
  { id: "3", name: "Katoomba" },
  { id: "4", name: "Wentworth Falls" },
];

/** Controlled harness so onChange actually updates what's rendered, like a real call site. */
function Harness({
  options = LOCATIONS,
  initial = new Set<string>(),
  name,
  onSubmit,
}: {
  options?: MultiSelectOption[];
  initial?: ReadonlySet<string>;
  name?: string;
  onSubmit?: () => void;
}) {
  const [value, setValue] = useState<ReadonlySet<string>>(initial);
  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        onSubmit?.();
      }}
    >
      <MultiSelectList
        options={options}
        value={value}
        onChange={setValue}
        name={name}
        itemLabel="locations"
      />
    </form>
  );
}

function getFilterInput() {
  return screen.getByRole("textbox", { name: "Filter locations" });
}

describe("MultiSelectList", () => {
  it("narrows visible rows by token match", async () => {
    const user = UserEvent.setup();
    render(<Harness />);

    await user.type(getFilterInput(), "pen nor");

    expect(
      screen.getByRole("checkbox", { name: /Penrith North/ }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("checkbox", { name: /Penrith South/ }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("checkbox", { name: /Katoomba/ }),
    ).not.toBeInTheDocument();
  });

  it("matches tokens regardless of order", async () => {
    const user = UserEvent.setup();
    render(<Harness />);

    await user.type(getFilterInput(), "north pen");

    expect(
      screen.getByRole("checkbox", { name: /Penrith North/ }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("checkbox", { name: /Penrith South/ }),
    ).not.toBeInTheDocument();
  });

  it("toggles the sole visible match on Enter without submitting the form", async () => {
    const user = UserEvent.setup();
    const onSubmit = vi.fn();
    render(<Harness onSubmit={onSubmit} />);

    await user.type(getFilterInput(), "pen nor");
    await user.keyboard("{Enter}");

    const checkbox = screen.getByRole("checkbox", {
      name: /Penrith North/,
    });
    expect(checkbox).toBeChecked();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("scopes 'Select N matching' to the filtered subset, leaving others untouched", async () => {
    const user = UserEvent.setup();
    render(<Harness initial={new Set(["3"])} />);

    await user.type(getFilterInput(), "penrith");
    await user.click(screen.getByRole("button", { name: "Select 2 matching" }));

    expect(
      screen.getByRole("checkbox", { name: /Penrith North/ }),
    ).toBeChecked();
    expect(
      screen.getByRole("checkbox", { name: /Penrith South/ }),
    ).toBeChecked();
    // Katoomba, selected before the filter was typed, is untouched — and
    // still selected even though it isn't currently visible.
    await user.clear(getFilterInput());
    expect(screen.getByRole("checkbox", { name: /Katoomba/ })).toBeChecked();
    expect(
      screen.getByRole("checkbox", { name: /Wentworth Falls/ }),
    ).not.toBeChecked();
  });

  it("'Clear all' zeroes the whole selection, not just the filtered slice", async () => {
    const user = UserEvent.setup();
    render(<Harness initial={new Set(["1", "3"])} />);

    await user.type(getFilterInput(), "penrith");
    await user.click(screen.getByRole("button", { name: /Clear all/ }));
    await user.clear(getFilterInput());

    for (const loc of LOCATIONS) {
      expect(
        screen.getByRole("checkbox", { name: new RegExp(loc.name) }),
      ).not.toBeChecked();
    }
  });

  it("moves focus into the list on ArrowDown, with only one tabbable row", async () => {
    const user = UserEvent.setup();
    render(<Harness />);

    getFilterInput().focus();
    await user.keyboard("{ArrowDown}");

    // Excludes the "Show selected only" toggle checkbox, which sits above
    // the list and isn't one of the roving-tabindex rows.
    const rowCheckboxes = screen
      .getAllByRole("checkbox")
      .filter((cb) =>
        LOCATIONS.some((loc) => cb.closest("label")?.textContent === loc.name),
      );

    expect(document.activeElement).toBe(rowCheckboxes[0]);

    const tabbable = rowCheckboxes.filter(
      (cb) => cb.getAttribute("tabindex") === "0",
    );
    expect(tabbable).toHaveLength(1);
    expect(tabbable[0]).toBe(rowCheckboxes[0]);
  });

  it("mirrors the selection with hidden inputs when `name` is passed", async () => {
    const user = UserEvent.setup();
    const { container } = render(<Harness name="locations" />);

    await user.click(screen.getByRole("checkbox", { name: /Katoomba/ }));
    await user.click(screen.getByRole("checkbox", { name: /Wentworth Falls/ }));

    const hiddenValues = [
      ...container.querySelectorAll<HTMLInputElement>(
        'input[type="hidden"][name="locations"]',
      ),
    ].map((el) => el.value);
    expect(hiddenValues.sort()).toEqual(["3", "4"]);

    // The visible checkboxes must not also carry `name`, or every selection
    // would be submitted twice via FormData.
    const visibleWithName = container.querySelectorAll(
      'input[type="checkbox"][name="locations"]',
    );
    expect(visibleWithName).toHaveLength(0);
  });

  it("still mirrors the selection with hidden inputs when disabled", () => {
    // A disabled <fieldset> drops its descendants from FormData, so the
    // hidden inputs must live outside it or a disabled selection would
    // silently vanish from the submitted form.
    const { container } = render(
      <MultiSelectList
        options={LOCATIONS}
        value={new Set(["3", "4"])}
        onChange={() => {}}
        name="locations"
        itemLabel="locations"
        disabled
      />,
    );

    const hiddenValues = [
      ...container.querySelectorAll<HTMLInputElement>(
        'input[type="hidden"][name="locations"]',
      ),
    ].map((el) => el.value);
    expect(hiddenValues.sort()).toEqual(["3", "4"]);
  });

  it("'Show selected only' narrows to the current selection", async () => {
    const user = UserEvent.setup();
    render(<Harness initial={new Set(["3"])} />);

    await user.click(
      screen.getByRole("checkbox", { name: /Show selected only/ }),
    );

    expect(
      screen.getByRole("checkbox", { name: /Katoomba/ }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("checkbox", { name: /Penrith North/ }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("checkbox", { name: /Wentworth Falls/ }),
    ).not.toBeInTheDocument();
  });

  it("shows emptyMessage when there are no options at all", () => {
    render(
      <MultiSelectList
        options={[]}
        value={new Set()}
        onChange={() => {}}
        itemLabel="locations"
        emptyMessage="No locations available to your account."
      />,
    );
    expect(
      screen.getByText("No locations available to your account."),
    ).toBeInTheDocument();
  });

  it("shows a no-match empty state when the filter matches nothing", async () => {
    const user = UserEvent.setup();
    render(<Harness />);

    await user.type(getFilterInput(), "zzzznotfound");

    expect(
      screen.getByText("No locations match “zzzznotfound”."),
    ).toBeInTheDocument();
  });
});

import { useEffect, useRef, useState } from "react";
import { inputBase, inputWidths, type InputWidth } from "./inputStyles";
import {
  comboboxChevron,
  comboboxClear,
  comboboxInput,
  comboboxWarning,
} from "./comboboxStyles";
import { filterOptions } from "./comboboxMatch";
import type { ComboboxOption } from "./comboboxMatch";
import ComboboxListbox from "./ComboboxListbox";
import { useComboboxList } from "./useComboboxList";
import { useMediaQuery } from "../useMediaQuery";
import Select from "./Select";

export type { ComboboxOption } from "./comboboxMatch";

/**
 * A single-select typeahead: a text box that filters a long option list as you
 * type, following the ARIA 1.2 editable-combobox pattern.
 *
 * **Which control to reach for.** Use the native `Select` when the options are a
 * fixed set defined in code, or when there are 15 or fewer of them with short
 * scannable labels. Use `Combobox` when the options come from the database, or
 * when the user knows the value *by name* rather than by scanning the list —
 * whichever the count. Prefer `SegmentedControl` at two or three options. Use
 * `MultiCombobox` — its sibling — when more than one value can be chosen.
 *
 * It works with the house form convention (`<form action>` reading `FormData`):
 * pass `name` and the selected option's `value` is submitted under it. See the
 * note on `required` below — it is the part most easily broken.
 *
 * The list-navigation plumbing (open/query/filtering/active row/popup
 * position) lives in `useComboboxList`, shared with `MultiCombobox`. What
 * stays here is specific to a *single* value: `Enter` and `Backspace` commit
 * or clear it, picking a row closes the list, and there is exactly one hidden
 * input.
 */
export interface ComboboxProps {
  /** Also the base for the listbox and option ids, so it must be unique. */
  id: string;
  /** `FormData` key. Omit for filter UIs that never submit. */
  name?: string;
  options: readonly ComboboxOption[];

  /** Controlled value; `""` means nothing selected. */
  value?: string;
  /** Uncontrolled initial value. */
  defaultValue?: string;
  onChange?: (value: string, option: ComboboxOption | null) => void;

  placeholder?: string;
  required?: boolean;
  disabled?: boolean;
  width?: InputWidth;
  className?: string;
  "aria-label"?: string;

  /** Defaults to `!required`. Shows a clear button and enables Backspace-to-clear. */
  allowClear?: boolean;
  /** Replaces the default ranked token matching. */
  filter?: (
    query: string,
    options: readonly ComboboxOption[],
  ) => readonly ComboboxOption[];
  /** Rendered as `{emptyText} “{query}”.` */
  emptyText?: string;

  /**
   * Render a native `<select>` below the `md` breakpoint instead, for the OS
   * picker on a phone. Off by default; on for the public period-edit form.
   */
  nativeOnSmallScreens?: boolean;
}

/** Tracks Tailwind's `md` breakpoint — keep the two in step. */
const DESKTOP_QUERY = "(min-width: 48rem)";

export default function Combobox({
  id,
  name,
  options,
  value,
  defaultValue,
  onChange,
  placeholder,
  required,
  disabled,
  width = "full",
  className,
  "aria-label": ariaLabel,
  allowClear = !required,
  filter = filterOptions,
  emptyText = "No matches",
  nativeOnSmallScreens = false,
}: ComboboxProps) {
  const [internal, setInternal] = useState(defaultValue ?? "");

  const wrapperRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listboxRef = useRef<HTMLUListElement>(null);

  const isDesktop = useMediaQuery(DESKTOP_QUERY);

  const list = useComboboxList({
    options,
    filter,
    idBase: id,
    anchorRef: wrapperRef,
    listboxRef,
  });

  const selected = value !== undefined ? value : internal;
  const selectedOption = options.find((o) => o.value === selected) ?? null;
  // A stored value whose option has since been deleted or filtered out. Rendering
  // an empty box here would be indistinguishable from "nothing selected", and the
  // user could save a different value without noticing, so say so instead.
  const missingOption = selected !== "" && selectedOption === null;

  // Closed, the box shows what is selected; open, it shows what you are typing.
  // Every close path resets `query`, so the label always comes back on its own.
  const displayValue = list.open
    ? list.query
    : (selectedOption?.label ?? selected);

  // Two cases where the box looks filled in but the value behind it isn't usable,
  // so native `required` — which only sees an empty box — would let them through.
  // Both must block submission rather than merely look wrong.
  useEffect(() => {
    const message = missingOption
      ? "This option is no longer available. Choose another."
      : required && !selected && displayValue !== ""
        ? "Select an option from the list."
        : "";
    inputRef.current?.setCustomValidity(message);
  }, [missingOption, required, selected, displayValue]);

  // React 19 resets an uncontrolled form once its action resolves, but the
  // selection lives in React state and would survive that. `useComboboxList`
  // resets its own open/query state on the same event.
  useEffect(() => {
    const form = wrapperRef.current?.closest("form");
    if (!form) return;
    function onReset() {
      setInternal(defaultValue ?? "");
    }
    form.addEventListener("reset", onReset);
    return () => form.removeEventListener("reset", onReset);
  }, [defaultValue]);

  function commit(next: string, option: ComboboxOption | null) {
    if (value === undefined) setInternal(next);
    onChange?.(next, option);
  }

  function select(option: ComboboxOption) {
    if (option.disabled) return;
    commit(option.value, option);
    list.close();
  }

  function clear() {
    commit("", null);
    list.search("");
    inputRef.current?.focus();
  }

  function getOpenIndex(fallbackToLast: boolean): number {
    const at = options.findIndex((o) => o.value === selected);
    if (at >= 0) return at;
    return fallbackToLast ? Math.max(0, options.length - 1) : 0;
  }

  function handleKeyDown(event: React.KeyboardEvent<HTMLInputElement>) {
    if (disabled) return;
    if (list.handleNavigationKey(event, getOpenIndex)) return;

    switch (event.key) {
      case "Enter":
        // Closed, Enter belongs to the form, exactly as with a native <select>.
        if (!list.open) return;
        event.preventDefault();
        // `activeIndex` is only negative when nothing matched, in which case
        // there is nothing to commit and Enter just dismisses the list.
        if (list.activeIndex >= 0) select(list.matches[list.activeIndex]);
        else list.close();
        return;
      case "Backspace":
        if (list.query === "" && allowClear && selected !== "") {
          event.preventDefault();
          commit("", null);
          list.search("");
        }
        return;
      // Home/End move the text caret: this is an *editable* combobox, so they
      // are text-editing keys. PageUp/PageDown cover bulk list movement.
      default:
        break;
    }

    // Typing while closed starts a fresh search rather than editing the label
    // that is sitting in the box, which is what a native <select>'s type-ahead
    // does and what the label being there would otherwise fight.
    if (
      !list.open &&
      event.key.length === 1 &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.altKey
    ) {
      event.preventDefault();
      list.search(event.key);
    }
  }

  function handleChange(event: React.ChangeEvent<HTMLInputElement>) {
    // Reachable while closed only by pasting over the displayed label, in which
    // case the pasted text becomes the query verbatim — visible, and editable.
    list.search(event.target.value);
  }

  if (nativeOnSmallScreens && !isDesktop) {
    return (
      <div ref={wrapperRef}>
        <Select
          id={id}
          name={name}
          required={required}
          disabled={disabled}
          width={width}
          className={className}
          aria-label={ariaLabel}
          {...(value !== undefined
            ? {
                value,
                onChange: (e) =>
                  commit(
                    e.target.value,
                    options.find((o) => o.value === e.target.value) ?? null,
                  ),
              }
            : { defaultValue: defaultValue ?? "" })}
        >
          <option value="">{placeholder ?? "Select…"}</option>
          {options.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
              {option.description ? ` (${option.description})` : ""}
            </option>
          ))}
        </Select>
      </div>
    );
  }

  const statusText = !list.open
    ? ""
    : list.matches.length === 0
      ? `${emptyText}.`
      : `${list.matches.length} option${list.matches.length === 1 ? "" : "s"} available.`;

  return (
    // The width lives on the wrapper, not the input: the chevron, the clear
    // button and the measured listbox width are all relative to it, and a
    // narrower input inside a full-width wrapper would leave all three hanging
    // off the right-hand edge.
    <div
      ref={wrapperRef}
      className={["relative inline-block", inputWidths[width]].join(" ")}
      onBlur={list.handleBlur}
    >
      <input
        ref={inputRef}
        id={id}
        type="text"
        role="combobox"
        aria-expanded={list.open}
        aria-controls={list.open ? `${id}-listbox` : undefined}
        aria-autocomplete="list"
        aria-activedescendant={list.activeId}
        aria-invalid={missingOption || undefined}
        aria-label={ariaLabel}
        value={displayValue}
        placeholder={placeholder}
        title={selectedOption?.label}
        required={required}
        disabled={disabled}
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
        spellCheck={false}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        onClick={() => (list.open ? list.close() : list.search(""))}
        className={[inputBase, comboboxInput, "w-full", className]
          .filter(Boolean)
          .join(" ")}
      />
      {/* Carries the value into FormData. Deliberately not the control that
          `required` sits on: a hidden input is barred from constraint
          validation, so `required` on it would silently never fire. */}
      {name && <input type="hidden" name={name} value={selected} />}
      {allowClear && selected !== "" && !disabled && (
        <button
          type="button"
          tabIndex={-1}
          aria-label="Clear selection"
          className={comboboxClear}
          onMouseDown={(e) => e.preventDefault()}
          onClick={clear}
        >
          ×
        </button>
      )}
      <span aria-hidden="true" className={comboboxChevron}>
        ▾
      </span>
      {missingOption && (
        <p className={comboboxWarning}>This option is no longer available.</p>
      )}
      <span role="status" className="sr-only">
        {statusText}
      </span>

      <ComboboxListbox
        idBase={id}
        list={list}
        listboxRef={listboxRef}
        isSelected={(option) => option.value === selected}
        onSelect={select}
        emptyText={emptyText}
        aria-label={ariaLabel}
      />
    </div>
  );
}

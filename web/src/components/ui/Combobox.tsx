import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { inputBase, inputWidths, type InputWidth } from "./inputStyles";
import {
  comboboxChevron,
  comboboxClear,
  comboboxDescription,
  comboboxEmpty,
  comboboxHighlight,
  comboboxInput,
  comboboxListbox,
  comboboxOption,
  comboboxOptionDisabled,
  comboboxOptionSelected,
  comboboxWarning,
} from "./comboboxStyles";
import { filterOptions, matchRanges } from "./comboboxMatch";
import type { ComboboxOption } from "./comboboxMatch";
import { useAnchoredPopup } from "./useAnchoredPopup";
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
 * whichever the count. Prefer `SegmentedControl` at two or three options.
 *
 * It works with the house form convention (`<form action>` reading `FormData`):
 * pass `name` and the selected option's `value` is submitted under it. See the
 * note on `required` below — it is the part most easily broken.
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

/** How far PageUp/PageDown move the active option. */
const PAGE = 10;

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
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [requestedActive, setRequestedActive] = useState(0);

  const wrapperRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listboxRef = useRef<HTMLUListElement>(null);

  const isDesktop = useMediaQuery(DESKTOP_QUERY);

  const selected = value !== undefined ? value : internal;
  const selectedOption = options.find((o) => o.value === selected) ?? null;
  // A stored value whose option has since been deleted or filtered out. Rendering
  // an empty box here would be indistinguishable from "nothing selected", and the
  // user could save a different value without noticing, so say so instead.
  const missingOption = selected !== "" && selectedOption === null;

  const matches = query.trim() === "" ? options : filter(query, options);
  // Derived rather than corrected in an effect: the list shrinks as the user
  // types, and the active row has to stay inside it on the very same render.
  const activeIndex = Math.min(requestedActive, matches.length - 1);

  // Closed, the box shows what is selected; open, it shows what you are typing.
  // Every close path resets `query`, so the label always comes back on its own.
  const displayValue = open ? query : (selectedOption?.label ?? selected);

  const position = useAnchoredPopup({
    open,
    onClose: () => close(),
    anchorRef: wrapperRef,
    popupRef: listboxRef,
    onViewportChange: "reposition",
  });

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
  // selection lives in React state and would survive that.
  useEffect(() => {
    const form = wrapperRef.current?.closest("form");
    if (!form) return;
    function onReset() {
      setInternal(defaultValue ?? "");
      setQuery("");
      setOpen(false);
    }
    form.addEventListener("reset", onReset);
    return () => form.removeEventListener("reset", onReset);
  }, [defaultValue]);

  useEffect(() => {
    if (!open || activeIndex < 0) return;
    // `nearest` only: `center` yanks the list on every arrow press. Optional
    // call because jsdom does not implement scrollIntoView.
    document
      .getElementById(`${id}-opt-${activeIndex}`)
      ?.scrollIntoView?.({ block: "nearest" });
  }, [open, activeIndex, id]);

  function close() {
    setOpen(false);
    setQuery("");
    setRequestedActive(0);
  }

  function commit(next: string, option: ComboboxOption | null) {
    if (value === undefined) setInternal(next);
    onChange?.(next, option);
  }

  function select(option: ComboboxOption) {
    if (option.disabled) return;
    commit(option.value, option);
    close();
  }

  function clear() {
    commit("", null);
    setQuery("");
    inputRef.current?.focus();
  }

  function moveActive(delta: number) {
    setRequestedActive((current) => {
      const from = Math.min(current, matches.length - 1);
      // No wrap-around: a native <select>, which these users are coming from,
      // doesn't wrap either, and jumping between the ends of 171 rows is
      // disorienting.
      return Math.max(0, Math.min(matches.length - 1, from + delta));
    });
  }

  function openAtSelection(fallbackToLast: boolean) {
    const at = options.findIndex((o) => o.value === selected);
    setRequestedActive(
      at >= 0 ? at : fallbackToLast ? Math.max(0, options.length - 1) : 0,
    );
    setOpen(true);
  }

  function handleKeyDown(event: React.KeyboardEvent<HTMLInputElement>) {
    if (disabled) return;

    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        if (open) moveActive(1);
        else openAtSelection(false);
        return;
      case "ArrowUp":
        event.preventDefault();
        if (!open) openAtSelection(true);
        else if (event.altKey) close();
        else moveActive(-1);
        return;
      case "PageDown":
        if (!open) return;
        event.preventDefault();
        moveActive(PAGE);
        return;
      case "PageUp":
        if (!open) return;
        event.preventDefault();
        moveActive(-PAGE);
        return;
      case "Enter":
        // Closed, Enter belongs to the form, exactly as with a native <select>.
        if (!open) return;
        event.preventDefault();
        // `activeIndex` is only negative when nothing matched, in which case
        // there is nothing to commit and Enter just dismisses the list.
        if (activeIndex >= 0) select(matches[activeIndex]);
        else close();
        return;
      case "Escape":
        // Only swallowed while the list is open, so it can still dismiss an
        // enclosing Dialog when it isn't. Closing resets the query, so there is
        // never a stale-text case to unwind here.
        if (!open) return;
        event.preventDefault();
        event.stopPropagation();
        close();
        return;
      case "Backspace":
        if (query === "" && allowClear && selected !== "") {
          event.preventDefault();
          commit("", null);
          setOpen(true);
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
      !open &&
      event.key.length === 1 &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.altKey
    ) {
      event.preventDefault();
      setQuery(event.key);
      setRequestedActive(0);
      setOpen(true);
    }
  }

  function handleChange(event: React.ChangeEvent<HTMLInputElement>) {
    // Reachable while closed only by pasting over the displayed label, in which
    // case the pasted text becomes the query verbatim — visible, and editable.
    setQuery(event.target.value);
    setRequestedActive(0);
    setOpen(true);
  }

  function handleBlur(event: React.FocusEvent<HTMLDivElement>) {
    if (event.currentTarget.contains(event.relatedTarget)) return;
    close();
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

  const statusText = !open
    ? ""
    : matches.length === 0
      ? `${emptyText}.`
      : `${matches.length} option${matches.length === 1 ? "" : "s"} available.`;

  return (
    // The width lives on the wrapper, not the input: the chevron, the clear
    // button and the measured listbox width are all relative to it, and a
    // narrower input inside a full-width wrapper would leave all three hanging
    // off the right-hand edge.
    <div
      ref={wrapperRef}
      className={["relative inline-block", inputWidths[width]].join(" ")}
      onBlur={handleBlur}
    >
      <input
        ref={inputRef}
        id={id}
        type="text"
        role="combobox"
        aria-expanded={open}
        aria-controls={open ? `${id}-listbox` : undefined}
        aria-autocomplete="list"
        aria-activedescendant={
          open && activeIndex >= 0 ? `${id}-opt-${activeIndex}` : undefined
        }
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
        onClick={() => (open ? close() : setOpen(true))}
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

      {open &&
        position &&
        createPortal(
          <ul
            ref={listboxRef}
            id={`${id}-listbox`}
            role="listbox"
            aria-label={ariaLabel}
            className={comboboxListbox}
            style={{
              position: "fixed",
              top: position.top,
              bottom: position.bottom,
              left: position.left,
              // Exactly the field's width, so the two always line up. Long
              // labels wrap rather than widening the popup past its anchor.
              width: position.width,
              minWidth: "16rem",
              maxHeight: position.maxHeight,
            }}
          >
            {matches.map((option, index) => (
              <li
                key={option.value}
                id={`${id}-opt-${index}`}
                role="option"
                aria-selected={option.value === selected}
                aria-disabled={option.disabled || undefined}
                data-active={index === activeIndex || undefined}
                // Keep focus in the input, so clicking never closes via blur.
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => select(option)}
                className={[
                  comboboxOption,
                  option.value === selected && comboboxOptionSelected,
                  option.disabled && comboboxOptionDisabled,
                ]
                  .filter(Boolean)
                  .join(" ")}
              >
                <Highlighted text={option.label} query={query} />
                {option.description && (
                  <span className={comboboxDescription}>
                    {/* Highlighted too: the description is searchable, so a
                        member found by number should show which number. */}
                    <Highlighted text={option.description} query={query} />
                  </span>
                )}
              </li>
            ))}
            {matches.length === 0 && (
              <li role="presentation" className={comboboxEmpty}>
                {emptyText} “{query}”.
              </li>
            )}
          </ul>,
          document.body,
        )}
    </div>
  );
}

/**
 * Emphasises the runs of `text` that `query` matched. These labels differ mostly
 * in their tail — twenty of them open "Workshop - Participant -" — so marking
 * the part that made a row a hit is what makes a filtered list scannable.
 */
function Highlighted({ text, query }: { text: string; query: string }) {
  const ranges = matchRanges(query, text);
  if (ranges.length === 0) return text;

  const parts: React.ReactNode[] = [];
  let at = 0;
  for (const [start, end] of ranges) {
    if (start > at) parts.push(text.slice(at, start));
    parts.push(
      <span key={start} className={comboboxHighlight}>
        {text.slice(start, end)}
      </span>,
    );
    at = end;
  }
  if (at < text.length) parts.push(text.slice(at));
  return parts;
}

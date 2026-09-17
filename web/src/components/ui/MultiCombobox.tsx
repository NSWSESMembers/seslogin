import { useEffect, useRef, useState } from "react";
import { inputBase, inputFocusWithin, inputWidths } from "./inputStyles";
import type { InputWidth } from "./inputStyles";
import {
  comboboxChevron,
  comboboxClear,
  comboboxPill,
  comboboxPillRemove,
  comboboxTokenBox,
  comboboxTokenInput,
} from "./comboboxStyles";
import { filterOptions } from "./comboboxMatch";
import type { ComboboxOption } from "./comboboxMatch";
import ComboboxListbox from "./ComboboxListbox";
import { useComboboxList } from "./useComboboxList";

export type { ComboboxOption } from "./comboboxMatch";

/**
 * A multi-select typeahead: a text box that filters a long option list as you
 * type, showing each choice as a removable pill. `Combobox`'s sibling — see
 * its doc comment for when to reach for which control.
 *
 * It works with the house form convention (`<form action>` reading
 * `FormData`): pass `name` and one hidden input is submitted per selection,
 * under that name, in the order the pills read — the same shape
 * `formData.getAll(name)` already expects from a list of checkboxes.
 *
 * Unlike `Combobox`, this control is **uncontrolled only**. It has no
 * controlled caller today, and a `value` prop with no caller would mean a
 * branch on every read of the selection and a code path only tests exercise.
 * Add it if a controlled caller shows up — the change is small.
 *
 * The list-navigation plumbing (open/query/filtering/active row/popup
 * position) lives in `useComboboxList`, shared with `Combobox`. What stays
 * here is specific to *multiple* values: `Enter` toggles the active option
 * without closing the list, `Backspace`/`Delete` on an empty query drops the
 * last pill, and there is one hidden input per selection.
 */
export interface MultiComboboxProps {
  /** Also the base for the listbox and option ids, so it must be unique. */
  id: string;
  /** `FormData` key. One hidden input is emitted per selection. */
  name?: string;
  options: readonly ComboboxOption[];

  /** Initial selection, in the order the pills should read. */
  defaultValue?: readonly string[];
  /** Fires after every add, remove, or clear. */
  onChange?: (values: string[], options: ComboboxOption[]) => void;

  placeholder?: string;
  /** "At least one option selected." See the note on validation below. */
  required?: boolean;
  disabled?: boolean;
  width?: InputWidth;
  className?: string;
  "aria-label"?: string;

  /** Defaults to `!required`. Shows a "Clear all" button. */
  allowClear?: boolean;
  /** Replaces the default ranked token matching. */
  filter?: (
    query: string,
    options: readonly ComboboxOption[],
  ) => readonly ComboboxOption[];
  /** Rendered as `{emptyText} “{query}”.` */
  emptyText?: string;
}

/** `defaultValue` entries with no matching option — e.g. a grant pointing at a
 * since-deleted location — are dropped rather than surfaced: no pill, no
 * hidden input, silently excluded from `onChange`. This matches what a
 * checkbox list already does with the same stale id (no checkbox exists for
 * it, so it is never submitted), and it's what every call site's data model
 * already assumes. */
function validIds(
  ids: readonly string[] | undefined,
  options: readonly ComboboxOption[],
): string[] {
  return (ids ?? []).filter((id) => options.some((o) => o.value === id));
}

export default function MultiCombobox({
  id,
  name,
  options,
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
}: MultiComboboxProps) {
  const [selectedIds, setSelectedIds] = useState<string[]>(() =>
    validIds(defaultValue, options),
  );
  // A pending live-region message that overrides the ordinary match-count
  // status until the next real search, so a pointer-driven add/remove/clear
  // is announced too — those never touch `list.query` if it was already
  // empty, so they can't just be derived from it.
  const [announcement, setAnnouncement] = useState("");

  const wrapperRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listboxRef = useRef<HTMLUListElement>(null);

  const list = useComboboxList({
    options,
    filter,
    idBase: id,
    anchorRef: wrapperRef,
    listboxRef,
  });

  const selected = selectedIds
    .map((v) => options.find((o) => o.value === v))
    .filter((o): o is ComboboxOption => o != null);

  // Not native `required`: the visible input holds the *query*, so a user who
  // typed and selected nothing would satisfy it while the field is actually
  // empty. A hidden input can't carry `required` either — hidden inputs are
  // barred from constraint validation, so it would silently never fire. So,
  // as with `Combobox`'s missing-option case, this blocks submission through
  // `setCustomValidity` on the visible input instead.
  useEffect(() => {
    inputRef.current?.setCustomValidity(
      required && selected.length === 0 ? "Select at least one option." : "",
    );
  }, [required, selected.length]);

  // React 19 resets an uncontrolled form once its action resolves, but the
  // selection lives in React state and would survive that. `useComboboxList`
  // resets its own open/query state on the same event.
  useEffect(() => {
    const form = wrapperRef.current?.closest("form");
    if (!form) return;
    function onReset() {
      setSelectedIds(validIds(defaultValue, options));
      setAnnouncement("");
    }
    form.addEventListener("reset", onReset);
    return () => form.removeEventListener("reset", onReset);
  }, [defaultValue, options]);

  function toggle(option: ComboboxOption) {
    if (option.disabled) return;
    const removing = selectedIds.includes(option.value);
    const nextIds = removing
      ? selectedIds.filter((v) => v !== option.value)
      : [...selectedIds, option.value];
    setSelectedIds(nextIds);
    onChange?.(
      nextIds,
      nextIds
        .map((v) => options.find((o) => o.value === v))
        .filter((o): o is ComboboxOption => o != null),
    );
    setAnnouncement(`${option.label} ${removing ? "removed" : "added"}.`);
    // Clears the query — matches becomes the full list again — and keeps the
    // just-toggled row active, so it stays under the cursor instead of
    // jumping back to the top of the list.
    const fullIndex = options.findIndex((o) => o.value === option.value);
    list.openAt(Math.max(0, fullIndex));
    inputRef.current?.focus();
  }

  function remove(option: ComboboxOption) {
    const nextIds = selectedIds.filter((v) => v !== option.value);
    setSelectedIds(nextIds);
    onChange?.(
      nextIds,
      nextIds
        .map((v) => options.find((o) => o.value === v))
        .filter((o): o is ComboboxOption => o != null),
    );
    setAnnouncement(`${option.label} removed.`);
  }

  function removeLast() {
    const last = selected.at(-1);
    if (last) remove(last);
  }

  function clearAll() {
    setSelectedIds([]);
    onChange?.([], []);
    setAnnouncement("Selection cleared.");
    inputRef.current?.focus();
  }

  function getOpenIndex(fallbackToLast: boolean): number {
    // No single value to jump back to, unlike `Combobox` — just the two ends
    // of the list, the same as when nothing is selected there.
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
        // there is nothing to toggle and Enter just dismisses the list.
        if (list.activeIndex >= 0) toggle(list.matches[list.activeIndex]);
        else list.close();
        return;
      case "Backspace":
      case "Delete":
        // Both keys: "delete" is Backspace on a Mac keyboard and Forward
        // Delete on a PC one, and reaching for either means the same thing.
        // Guarded on an empty query either way — with text in the box both
        // must stay text-editing keys, and at the end of a query there is no
        // forward character for Delete to eat, so unguarded it would
        // silently destroy a pill mid-word.
        if (list.query === "" && selected.length > 0) {
          event.preventDefault();
          removeLast();
        }
        return;
      // Home/End move the text caret: this is an *editable* combobox, so they
      // are text-editing keys. PageUp/PageDown cover bulk list movement.
      default:
        break;
    }

    // Typing while closed starts a fresh search, as with `Combobox`.
    if (
      !list.open &&
      event.key.length === 1 &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.altKey
    ) {
      event.preventDefault();
      setAnnouncement("");
      list.search(event.key);
    }
  }

  function handleChange(event: React.ChangeEvent<HTMLInputElement>) {
    setAnnouncement("");
    list.search(event.target.value);
  }

  const listStatusText = !list.open
    ? ""
    : list.matches.length === 0
      ? `${emptyText}.`
      : `${list.matches.length} option${list.matches.length === 1 ? "" : "s"} available.`;
  const statusText = announcement || listStatusText;

  return (
    <div
      ref={wrapperRef}
      className={[
        inputBase,
        inputFocusWithin,
        comboboxTokenBox,
        "relative",
        inputWidths[width],
        disabled && "opacity-60",
        className,
      ]
        .filter(Boolean)
        .join(" ")}
      onBlur={list.handleBlur}
      onClick={(e) => {
        // Clicking the box's own background (the gaps around and after the
        // pills) focuses the input, the way clicking a native text input
        // anywhere in its padding does. A click on a pill or its × button
        // never reaches here — those are earlier siblings, not this div.
        if (e.target === e.currentTarget) inputRef.current?.focus();
      }}
    >
      {selected.map((option) => (
        <span key={option.value} className={comboboxPill}>
          {option.label}
          <button
            type="button"
            aria-label={`Remove ${option.label}`}
            className={comboboxPillRemove}
            disabled={disabled}
            // Keep focus in the input: a mouse click removes the pill without
            // ever moving focus to its button, matching how the listbox rows
            // and the clear button below keep focus put.
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => remove(option)}
          >
            ×
          </button>
        </span>
      ))}
      <input
        ref={inputRef}
        id={id}
        type="text"
        role="combobox"
        aria-expanded={list.open}
        aria-controls={list.open ? `${id}-listbox` : undefined}
        aria-autocomplete="list"
        aria-activedescendant={list.activeId}
        aria-required={required || undefined}
        aria-label={ariaLabel}
        value={list.query}
        placeholder={selected.length === 0 ? placeholder : undefined}
        disabled={disabled}
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
        spellCheck={false}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        onClick={() => (list.open ? list.close() : list.search(""))}
        className={comboboxTokenInput}
      />
      {/* Carries the selection into FormData, one entry per pill — exactly
          what `formData.getAll(name)` already expects from a checkbox list. */}
      {name &&
        selected.map((option) => (
          <input
            key={option.value}
            type="hidden"
            name={name}
            value={option.value}
          />
        ))}
      {allowClear && selected.length > 0 && !disabled && (
        <button
          type="button"
          tabIndex={-1}
          aria-label="Clear all"
          className={comboboxClear}
          onMouseDown={(e) => e.preventDefault()}
          onClick={clearAll}
        >
          ×
        </button>
      )}
      <span aria-hidden="true" className={comboboxChevron}>
        ▾
      </span>
      <span role="status" className="sr-only">
        {statusText}
      </span>

      <ComboboxListbox
        idBase={id}
        list={list}
        listboxRef={listboxRef}
        isSelected={(option) => selected.some((o) => o.value === option.value)}
        onSelect={toggle}
        multiselectable
        emptyText={emptyText}
        aria-label={ariaLabel}
      />
    </div>
  );
}

import { createPortal } from "react-dom";
import type { RefObject } from "react";
import type { ComboboxOption } from "./comboboxMatch";
import { matchRanges } from "./comboboxMatch";
import type { ComboboxList } from "./useComboboxList";
import {
  comboboxDescription,
  comboboxEmpty,
  comboboxHighlight,
  comboboxListbox,
  comboboxOption,
  comboboxOptionCheck,
  comboboxOptionDisabled,
  comboboxOptionSelected,
} from "./comboboxStyles";

export interface ComboboxListboxProps {
  /** Must match the `idBase` passed to the `useComboboxList` that produced `list`. */
  idBase: string;
  list: ComboboxList;
  /** The same ref passed as `listboxRef` to that `useComboboxList` call. */
  listboxRef: RefObject<HTMLUListElement | null>;
  isSelected: (option: ComboboxOption) => boolean;
  onSelect: (option: ComboboxOption) => void;
  /** Adds `aria-multiselectable` and a ✓ marker on selected rows. */
  multiselectable?: boolean;
  /** Rendered as `{emptyText} "{query}".` */
  emptyText: string;
  "aria-label"?: string;
}

/**
 * The portalled `<ul role="listbox">` behind both `Combobox` and
 * `MultiCombobox`. Rendering, not selection: which rows are selected and what
 * clicking one does are entirely the caller's business via `isSelected` and
 * `onSelect`.
 */
export default function ComboboxListbox({
  idBase,
  list,
  listboxRef,
  isSelected,
  onSelect,
  multiselectable,
  emptyText,
  "aria-label": ariaLabel,
}: ComboboxListboxProps) {
  if (!list.open || !list.position) return null;

  return createPortal(
    <ul
      ref={listboxRef}
      id={`${idBase}-listbox`}
      role="listbox"
      aria-multiselectable={multiselectable || undefined}
      aria-label={ariaLabel}
      className={comboboxListbox}
      style={{
        position: "fixed",
        top: list.position.top,
        bottom: list.position.bottom,
        left: list.position.left,
        // Exactly the field's width, so the two always line up. Long labels
        // wrap rather than widening the popup past its anchor.
        width: list.position.width,
        minWidth: "16rem",
        maxHeight: list.position.maxHeight,
      }}
    >
      {list.matches.map((option, index) => {
        const selected = isSelected(option);
        return (
          <li
            key={option.value}
            id={`${idBase}-opt-${index}`}
            role="option"
            aria-selected={selected}
            aria-disabled={option.disabled || undefined}
            data-active={index === list.activeIndex || undefined}
            // Keep focus in the input, so clicking never closes via blur.
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => onSelect(option)}
            className={[
              comboboxOption,
              selected && comboboxOptionSelected,
              option.disabled && comboboxOptionDisabled,
            ]
              .filter(Boolean)
              .join(" ")}
          >
            {multiselectable && (
              <span aria-hidden="true" className={comboboxOptionCheck}>
                {selected ? "✓" : ""}
              </span>
            )}
            <Highlighted text={option.label} query={list.query} />
            {option.description && (
              <span className={comboboxDescription}>
                {/* Highlighted too: the description is searchable, so a
                    member found by number should show which number. */}
                <Highlighted text={option.description} query={list.query} />
              </span>
            )}
          </li>
        );
      })}
      {list.matches.length === 0 && (
        <li role="presentation" className={comboboxEmpty}>
          {emptyText} “{list.query}”.
        </li>
      )}
    </ul>,
    document.body,
  );
}

/**
 * Emphasises the runs of `text` that `query` matched. These labels differ
 * mostly in their tail — twenty of them open "Workshop - Participant -" — so
 * marking the part that made a row a hit is what makes a filtered list
 * scannable.
 */
export function Highlighted({ text, query }: { text: string; query: string }) {
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

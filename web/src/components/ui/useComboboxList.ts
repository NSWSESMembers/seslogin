import { useEffect, useState } from "react";
import type { RefObject } from "react";
import { filterOptions } from "./comboboxMatch";
import type { ComboboxOption } from "./comboboxMatch";
import { useAnchoredPopup } from "./useAnchoredPopup";
import type { AnchoredPosition } from "./useAnchoredPopup";

/**
 * The list-navigation half of an editable combobox — what's open, what's
 * typed, what's filtered, which row is active, and where the popup sits.
 * Shared by `Combobox` (single-select) and `MultiCombobox` (pills), which
 * differ only in what a selected *value* means: what `Enter` and
 * `Backspace`/`Delete` do, whether picking a row closes the list, and how the
 * chosen value(s) reach `FormData`. Those stay in each component; this hook
 * knows nothing about selection.
 */

/** How far PageUp/PageDown move the active option. */
const PAGE = 10;

export interface UseComboboxListOptions {
  options: readonly ComboboxOption[];
  /** Replaces the default ranked token matching. */
  filter?: (
    query: string,
    options: readonly ComboboxOption[],
  ) => readonly ComboboxOption[];
  /** Base for the listbox and option ids, so it must be unique. */
  idBase: string;
  anchorRef: RefObject<HTMLElement | null>;
  /**
   * Owned by the caller and passed straight through to `ComboboxListbox`, the
   * same way `useAnchoredPopup` takes `anchorRef`/`popupRef` rather than
   * creating them: the compiler's ref analysis cannot follow a ref through a
   * custom hook's return value, so one created and returned here would make
   * every other field on this hook's result look ref-tainted too.
   */
  listboxRef: RefObject<HTMLUListElement | null>;
}

export interface ComboboxList {
  open: boolean;
  query: string;
  matches: readonly ComboboxOption[];
  /** Derived rather than corrected in an effect: the list shrinks as the user
   * types, and the active row has to stay inside it on the very same render.
   * `-1` when nothing matches. */
  activeIndex: number;
  /** For `aria-activedescendant`; `undefined` when closed or nothing is active. */
  activeId: string | undefined;
  position: AnchoredPosition | null;

  /**
   * Clears the query (so `matches` becomes the full option list), moves the
   * active row to `index` within it, and opens. Used both to jump to a
   * starting row when opening from closed, and — since a fresh query is
   * exactly what committing a value needs anyway — to keep the just-acted-on
   * row active after a selection clears the query.
   */
  openAt: (index: number) => void;
  /** Closes, resetting the query and active row. */
  close: () => void;
  /** Sets the query, resets the active row to the top, and opens. */
  search: (query: string) => void;
  /**
   * Handles ArrowUp/ArrowDown (including Alt+ArrowUp to close), PageUp/
   * PageDown, and Escape (swallowed only while open, so it can still reach an
   * enclosing Dialog when closed). `getOpenIndex` supplies the row to land on
   * when an arrow key opens the list from closed — `Combobox` jumps to the
   * current value, `MultiCombobox` has no single value to jump to. Returns
   * whether the key was consumed, so the caller can fall through to the keys
   * that carry value meaning (`Enter`, `Backspace`, printable characters).
   */
  handleNavigationKey: (
    event: React.KeyboardEvent,
    getOpenIndex: (fallbackToLast: boolean) => number,
  ) => boolean;
  /** Closes when focus leaves the anchor entirely, e.g. via Tab. */
  handleBlur: (event: React.FocusEvent) => void;
}

export function useComboboxList({
  options,
  filter = filterOptions,
  idBase,
  anchorRef,
  listboxRef,
}: UseComboboxListOptions): ComboboxList {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [requestedActive, setRequestedActive] = useState(0);

  const matches = query.trim() === "" ? options : filter(query, options);
  const activeIndex = Math.min(requestedActive, matches.length - 1);
  const activeId =
    open && activeIndex >= 0 ? `${idBase}-opt-${activeIndex}` : undefined;

  const position = useAnchoredPopup({
    open,
    onClose: () => close(),
    anchorRef,
    popupRef: listboxRef,
    onViewportChange: "reposition",
    // A 288px-tall listbox on a field low in the viewport has to go above it.
    flip: true,
  });

  useEffect(() => {
    if (!open || activeIndex < 0) return;
    // `nearest` only: `center` yanks the list on every arrow press. Optional
    // call because jsdom does not implement scrollIntoView.
    document
      .getElementById(`${idBase}-opt-${activeIndex}`)
      ?.scrollIntoView?.({ block: "nearest" });
  }, [open, activeIndex, idBase]);

  // React 19 resets an uncontrolled form once its action resolves, but `open`
  // and `query` live in this hook's own state and would survive that. Each
  // caller adds its own listener alongside this one to reset its selected
  // value(s); both fire off the same native `reset` event.
  useEffect(() => {
    const form = anchorRef.current?.closest("form");
    if (!form) return;
    function onReset() {
      setOpen(false);
      setQuery("");
      setRequestedActive(0);
    }
    form.addEventListener("reset", onReset);
    return () => form.removeEventListener("reset", onReset);
  }, [anchorRef]);

  function close() {
    setOpen(false);
    setQuery("");
    setRequestedActive(0);
  }

  function openAt(index: number) {
    setQuery("");
    setRequestedActive(index);
    setOpen(true);
  }

  function search(nextQuery: string) {
    setQuery(nextQuery);
    setRequestedActive(0);
    setOpen(true);
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

  function handleNavigationKey(
    event: React.KeyboardEvent,
    getOpenIndex: (fallbackToLast: boolean) => number,
  ): boolean {
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        if (open) moveActive(1);
        else openAt(getOpenIndex(false));
        return true;
      case "ArrowUp":
        event.preventDefault();
        if (!open) openAt(getOpenIndex(true));
        else if (event.altKey) close();
        else moveActive(-1);
        return true;
      case "PageDown":
        if (!open) return false;
        event.preventDefault();
        moveActive(PAGE);
        return true;
      case "PageUp":
        if (!open) return false;
        event.preventDefault();
        moveActive(-PAGE);
        return true;
      case "Escape":
        // Only swallowed while the list is open, so it can still dismiss an
        // enclosing Dialog when it isn't. Closing resets the query, so there
        // is never a stale-text case to unwind here.
        if (!open) return false;
        event.preventDefault();
        event.stopPropagation();
        close();
        return true;
      default:
        return false;
    }
  }

  function handleBlur(event: React.FocusEvent) {
    if (event.currentTarget.contains(event.relatedTarget)) return;
    close();
  }

  return {
    open,
    query,
    matches,
    activeIndex,
    activeId,
    position,
    openAt,
    close,
    search,
    handleNavigationKey,
    handleBlur,
  };
}

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { RefObject } from "react";

/**
 * Position for a popup portalled to `<body>` with `position: fixed`. Exactly one
 * of `top`/`bottom` is set: anchoring to `bottom` when the popup opens upwards
 * lets it grow away from the anchor without anyone measuring its content.
 *
 * `width` and `maxHeight` are advisory — a `w-max` tooltip ignores both, while a
 * listbox matches the field's width and clamps its height.
 */
export interface AnchoredPosition {
  top?: number;
  bottom?: number;
  left: number;
  width: number;
  maxHeight: number;
}

interface UseAnchoredPopupOptions {
  open: boolean;
  onClose: () => void;
  anchorRef: RefObject<HTMLElement | null>;
  popupRef: RefObject<HTMLElement | null>;
  /**
   * What a scroll or viewport resize means. `"close"` suits a tooltip, which
   * would otherwise detach from its trigger. A combobox must `"reposition"`:
   * focusing its input on iOS raises the keyboard, which fires a
   * `visualViewport` resize and often a document scroll, so closing on those
   * would slam the list shut the instant it was tapped.
   */
  onViewportChange?: "close" | "reposition";
  /** Cap on the popup's height, in px. Defaults to 288 (Tailwind's `max-h-72`). */
  maxHeight?: number;
  /**
   * `"start"` left-aligns `left` to the anchor. `"center"` returns the anchor's
   * midpoint instead, for a caller that carries its own `-translate-x-1/2`.
   */
  align?: "start" | "center";
  /**
   * Dismiss on Escape. Off by default, for callers that handle the key
   * themselves — a combobox only swallows Escape while its list is open, so
   * that it can still reach an enclosing Dialog when closed.
   */
  closeOnEscape?: boolean;
  /**
   * Flip above the anchor when there isn't room below. Off by default so a
   * popup only starts moving when its caller asks it to.
   */
  flip?: boolean;
  /** Gap between anchor and popup, px. */
  gap?: number;
}

/** Below this much room underneath the anchor, a flipping popup goes above it. */
const FLIP_THRESHOLD = 200;
/** Never squash the popup below this, even in a very short viewport. */
const MIN_HEIGHT = 96;

/**
 * Measures an anchor, keeps a portalled popup positioned against it, and closes
 * the popup on an outside click.
 *
 * The measure/dismiss engine behind both `Popover` — the click-toggled tooltip
 * used by `FingerprintChip` and `CommentIndicator` — and `Combobox`'s listbox.
 * The defaults are the tooltip's behaviour; the listbox opts into flipping and
 * into repositioning rather than closing.
 */
export function useAnchoredPopup({
  open,
  onClose,
  anchorRef,
  popupRef,
  onViewportChange = "close",
  maxHeight: maxHeightCap = 288,
  align = "start",
  closeOnEscape = false,
  flip = false,
  gap = 4,
}: UseAnchoredPopupOptions): AnchoredPosition | null {
  const [position, setPosition] = useState<AnchoredPosition | null>(null);

  // `onClose` is typically an inline closure, so depending on it directly would
  // re-run the effects every render — and since measuring sets state, that would
  // not terminate.
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  useLayoutEffect(() => {
    if (!open) return;
    const anchor = anchorRef.current;
    if (!anchor) return;

    function measure() {
      const rect = anchor!.getBoundingClientRect();
      // `visualViewport` is the part not covered by an on-screen keyboard;
      // `innerHeight` does not shrink for it. Undefined in jsdom.
      const viewportHeight =
        window.visualViewport?.height ?? window.innerHeight;
      const below = viewportHeight - rect.bottom;
      const above = rect.top;
      const placeAbove = flip && below < FLIP_THRESHOLD && above > below;
      const room = (placeAbove ? above : below) - gap * 2;
      setPosition({
        top: placeAbove ? undefined : rect.bottom + gap,
        bottom: placeAbove ? viewportHeight - rect.top + gap : undefined,
        left: align === "center" ? rect.left + rect.width / 2 : rect.left,
        width: rect.width,
        maxHeight: Math.max(MIN_HEIGHT, Math.min(maxHeightCap, room)),
      });
    }

    function handleViewportChange(event: Event) {
      // A scrollable popup scrolling itself is not the viewport moving. Without
      // this, a capture-phase window listener sees it and closes the popup the
      // first time the user scrolls the list.
      if (
        event.target instanceof Node &&
        popupRef.current?.contains(event.target)
      ) {
        return;
      }
      if (onViewportChange === "close") {
        onCloseRef.current();
      } else {
        measure();
      }
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") onCloseRef.current();
    }

    measure();
    // Capture phase so scrolling ancestors are caught, not just the document.
    window.addEventListener("scroll", handleViewportChange, true);
    window.addEventListener("resize", handleViewportChange);
    window.visualViewport?.addEventListener("resize", handleViewportChange);
    window.visualViewport?.addEventListener("scroll", handleViewportChange);
    if (closeOnEscape) document.addEventListener("keydown", handleKeyDown);
    // The anchor can resize on its own while open with none of the above
    // firing — e.g. a token box growing a second line as pills are added.
    // Without this, `position` keeps the anchor's old (shorter) rect and the
    // popup stays put, now overlapping the anchor's newly-grown content.
    // Unimplemented in jsdom, so this is a no-op there — geometry isn't
    // asserted in tests either way (see `Combobox.test.tsx`).
    const resizeObserver =
      typeof ResizeObserver === "function"
        ? new ResizeObserver(() => measure())
        : undefined;
    resizeObserver?.observe(anchor);
    return () => {
      window.removeEventListener("scroll", handleViewportChange, true);
      window.removeEventListener("resize", handleViewportChange);
      window.visualViewport?.removeEventListener(
        "resize",
        handleViewportChange,
      );
      window.visualViewport?.removeEventListener(
        "scroll",
        handleViewportChange,
      );
      document.removeEventListener("keydown", handleKeyDown);
      resizeObserver?.disconnect();
    };
  }, [
    open,
    anchorRef,
    popupRef,
    onViewportChange,
    maxHeightCap,
    align,
    closeOnEscape,
    flip,
    gap,
  ]);

  useEffect(() => {
    if (!open) return;
    function onPointerDown(event: MouseEvent) {
      const target = event.target as Node;
      if (
        !anchorRef.current?.contains(target) &&
        !popupRef.current?.contains(target)
      ) {
        onCloseRef.current();
      }
    }
    document.addEventListener("mousedown", onPointerDown);
    return () => document.removeEventListener("mousedown", onPointerDown);
  }, [open, anchorRef, popupRef]);

  // The last measurement is kept while closed rather than cleared, so that
  // clearing it isn't a setState in an effect body. Gated here instead: a caller
  // never sees a position for a popup that isn't open, and the layout effect
  // re-measures before the reopened popup paints.
  return open ? position : null;
}

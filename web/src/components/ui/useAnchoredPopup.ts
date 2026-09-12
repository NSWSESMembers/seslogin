import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { RefObject } from "react";

/**
 * Position for a popup portalled to `<body>` with `position: fixed`. Exactly one
 * of `top`/`bottom` is set: anchoring to `bottom` when the popup opens upwards
 * lets it grow away from the anchor without anyone measuring its content.
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
}

/** Gap between anchor and popup, px. */
const GAP = 4;
/** Below this much room underneath the anchor, the popup flips above it. */
const FLIP_THRESHOLD = 200;
/** Never squash the popup below this, even in a very short viewport. */
const MIN_HEIGHT = 96;

/**
 * Measures an anchor and keeps a portalled popup positioned against it, and
 * closes the popup on an outside click.
 *
 * Generalises the pattern hand-rolled in `FingerprintChip` and
 * `CommentIndicator`; neither has been moved onto it yet.
 */
export function useAnchoredPopup({
  open,
  onClose,
  anchorRef,
  popupRef,
  onViewportChange = "close",
  maxHeight: maxHeightCap = 288,
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
      const placeAbove = below < FLIP_THRESHOLD && above > below;
      const room = (placeAbove ? above : below) - GAP * 2;
      setPosition({
        top: placeAbove ? undefined : rect.bottom + GAP,
        bottom: placeAbove ? viewportHeight - rect.top + GAP : undefined,
        left: rect.left,
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

    measure();
    // Capture phase so scrolling ancestors are caught, not just the document.
    window.addEventListener("scroll", handleViewportChange, true);
    window.addEventListener("resize", handleViewportChange);
    window.visualViewport?.addEventListener("resize", handleViewportChange);
    window.visualViewport?.addEventListener("scroll", handleViewportChange);
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
    };
  }, [open, anchorRef, popupRef, onViewportChange, maxHeightCap]);

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

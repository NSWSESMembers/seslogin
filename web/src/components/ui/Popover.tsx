import { useRef } from "react";
import type { ReactNode, RefObject } from "react";
import { createPortal } from "react-dom";
import { useAnchoredPopup } from "./useAnchoredPopup";

// A small floating panel anchored under a trigger element — a click-toggled
// popover, not a Dialog: no backdrop, positioned next to the trigger rather
// than centred. Portalled to <body> with fixed positioning so it isn't
// clipped by a scrolling table cell or dialog it happens to be rendered
// inside (see Dialog's comment for the same reasoning).
//
// Only mounted while open, like <Dialog> — the caller owns the open/closed
// state and passes `onDismiss` to close it. Closes itself on outside
// click, Escape, or any scroll/resize (a fixed-position panel would
// otherwise detach from its anchor).
//
// The measuring and dismissing is `useAnchoredPopup`, shared with Combobox's
// listbox. This is the tooltip half of it: centred on the anchor, sized to its
// content, and closing rather than following when the viewport moves.
export function Popover({
  anchorRef,
  onDismiss,
  role = "tooltip",
  gap = 4,
  className,
  children,
}: {
  anchorRef: RefObject<HTMLElement | null>;
  onDismiss: () => void;
  role?: string;
  gap?: number;
  className?: string;
  children: ReactNode;
}) {
  const panelRef = useRef<HTMLDivElement>(null);
  const pos = useAnchoredPopup({
    open: true,
    onClose: onDismiss,
    anchorRef,
    popupRef: panelRef,
    align: "center",
    closeOnEscape: true,
    gap,
  });

  if (!pos) return null;

  return createPortal(
    <div
      ref={panelRef}
      role={role}
      style={{ top: pos.top, left: pos.left }}
      className={`fixed z-50 w-max -translate-x-1/2 rounded-md border border-line bg-surface-raised text-left text-ink shadow-lg ${className ?? ""}`}
    >
      {children}
    </div>,
    document.body,
  );
}

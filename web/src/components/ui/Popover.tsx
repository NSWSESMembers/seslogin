import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ReactNode, RefObject } from "react";
import { createPortal } from "react-dom";

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
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);

  useLayoutEffect(() => {
    const anchor = anchorRef.current;
    if (!anchor) return;
    const rect = anchor.getBoundingClientRect();
    setPos({ top: rect.bottom + gap, left: rect.left + rect.width / 2 });
  }, [anchorRef, gap]);

  useEffect(() => {
    function onPointerDown(e: MouseEvent) {
      const target = e.target as Node;
      if (
        !anchorRef.current?.contains(target) &&
        !panelRef.current?.contains(target)
      ) {
        onDismiss();
      }
    }
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") onDismiss();
    }
    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    window.addEventListener("scroll", onDismiss, true);
    window.addEventListener("resize", onDismiss);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("scroll", onDismiss, true);
      window.removeEventListener("resize", onDismiss);
    };
  }, [anchorRef, onDismiss]);

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

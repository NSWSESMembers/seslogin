import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

// A speech-bubble icon shown on a period row when it has a comment. The comment
// text is revealed on hover (native title tooltip) and on press/click (a small
// popover, so it also works on touch devices without hover).
//
// The popover is portalled to document.body with fixed positioning so it isn't
// clipped by the table's `overflow-x-auto` scroll container.
export default function CommentIndicator({ comment }: { comment: string }) {
  const [open, setOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);

  // Position the popover under the icon once it's rendered and measurable.
  useLayoutEffect(() => {
    if (!open) return;
    const button = buttonRef.current;
    if (!button) return;
    const rect = button.getBoundingClientRect();
    setPos({ top: rect.bottom + 4, left: rect.left + rect.width / 2 });
  }, [open]);

  // Close the popover on outside click, Escape, or any scroll/resize (a fixed
  // popover would otherwise detach from the icon).
  useEffect(() => {
    if (!open) return;
    function onPointerDown(e: MouseEvent) {
      const target = e.target as Node;
      if (
        !buttonRef.current?.contains(target) &&
        !popoverRef.current?.contains(target)
      ) {
        setOpen(false);
      }
    }
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    function close() {
      setOpen(false);
    }
    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    window.addEventListener("scroll", close, true);
    window.addEventListener("resize", close);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("scroll", close, true);
      window.removeEventListener("resize", close);
    };
  }, [open]);

  return (
    <>
      <button
        ref={buttonRef}
        type="button"
        title={comment}
        aria-label={open ? "Hide comment" : "Show comment"}
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="cursor-help align-middle text-ink-muted transition-colors hover:text-menu"
      >
        <svg
          viewBox="0 0 16 16"
          width={14}
          height={14}
          fill="currentColor"
          aria-hidden="true"
          className="align-middle"
        >
          <path d="M3 2h10a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1H6.5l-3 3v-3H3a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1zm1.5 3a.75.75 0 0 0 0 1.5h7a.75.75 0 0 0 0-1.5h-7zm0 3a.75.75 0 0 0 0 1.5h4a.75.75 0 0 0 0-1.5h-4z" />
        </svg>
      </button>
      {open &&
        pos &&
        createPortal(
          <div
            ref={popoverRef}
            role="tooltip"
            style={{ top: pos.top, left: pos.left }}
            className="fixed z-50 w-max max-w-xs -translate-x-1/2 rounded-md border border-line bg-surface-raised px-2 py-1.5 text-left text-sm whitespace-pre-wrap text-ink shadow-lg"
          >
            {comment}
          </div>,
          document.body,
        )}
    </>
  );
}

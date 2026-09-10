import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { groupFingerprint, shortFingerprint } from "../lib/fingerprint";

// The truncated fingerprint as an inline control: the full value is on the
// `title` (hover) and in a popover on click, so it also works on a touch kiosk
// with no pointer. The popover is portalled to <body> with fixed positioning so
// it isn't clipped by a dialog or the sessions table's scroll container.
export function FingerprintChip({
  fingerprint,
  className,
}: {
  fingerprint: string;
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);

  useLayoutEffect(() => {
    if (!open) return;
    const button = buttonRef.current;
    if (!button) return;
    const rect = button.getBoundingClientRect();
    setPos({ top: rect.bottom + 4, left: rect.left + rect.width / 2 });
  }, [open]);

  // Close on outside click, Escape, or any scroll/resize (a fixed popover would
  // otherwise detach from the chip).
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
        title={fingerprint}
        aria-label={open ? "Hide full device key" : "Show full device key"}
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className={`cursor-help font-mono break-all underline decoration-dotted underline-offset-2 ${className ?? ""}`}
      >
        {shortFingerprint(fingerprint)}
      </button>
      {open &&
        pos &&
        createPortal(
          <div
            ref={popoverRef}
            role="tooltip"
            style={{ top: pos.top, left: pos.left }}
            className="fixed z-50 w-max max-w-[min(20rem,92vw)] -translate-x-1/2 rounded-md border border-line bg-surface-raised px-3 py-2 text-left text-ink shadow-lg"
          >
            <p className="m-0 text-xs font-semibold text-ink-muted uppercase">
              Full device key
            </p>
            {/* No `break-all`: `groupFingerprint` puts a space between every
                4-hex group, so normal word wrapping breaks the line at those
                spaces and never mid-group. */}
            <p className="m-0 font-mono text-sm select-all">
              {groupFingerprint(fingerprint)}
            </p>
          </div>,
          document.body,
        )}
    </>
  );
}

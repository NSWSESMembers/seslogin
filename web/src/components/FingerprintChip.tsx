import { useRef, useState } from "react";
import { groupFingerprint, shortFingerprint } from "../lib/fingerprint";
import { Popover } from "./ui/Popover";

// The truncated fingerprint as an inline control: the full value is on the
// `title` (hover) and in a popover on click, so it also works on a touch kiosk
// with no pointer.
export function FingerprintChip({
  fingerprint,
  className,
}: {
  fingerprint: string;
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);

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
      {open && (
        <Popover
          anchorRef={buttonRef}
          onDismiss={() => setOpen(false)}
          className="max-w-[min(20rem,92vw)] px-3 py-2"
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
        </Popover>
      )}
    </>
  );
}

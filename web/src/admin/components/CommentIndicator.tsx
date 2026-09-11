import { useRef, useState } from "react";
import { Popover } from "../../components/ui/Popover";

// A speech-bubble icon shown on a period row when it has a comment. The comment
// text is revealed on hover (native title tooltip) and on press/click (a small
// popover, so it also works on touch devices without hover).
export default function CommentIndicator({ comment }: { comment: string }) {
  const [open, setOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);

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
      {open && (
        <Popover
          anchorRef={buttonRef}
          onDismiss={() => setOpen(false)}
          className="max-w-xs px-2 py-1.5 text-sm whitespace-pre-wrap"
        >
          {comment}
        </Popover>
      )}
    </>
  );
}

import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { cn } from "../../lib/tw";

// Centred modal dialog over a dimmed backdrop, both fading in on mount.
// Clicking the backdrop calls onDismiss (omit it to make the dialog
// non-dismissable). `width` and `className` are merged via cn(), so a width
// utility passed in `className` overrides the default `w-150` correctly.
//
// Portalled to <body> so the panel isn't affected by layout or `white-space`
// in whatever DOM position it's opened from — a Dialog opened from inside
// e.g. a `whitespace-nowrap` table cell would otherwise inherit that and
// mangle its own contents. Sets `text-center` explicitly (rather than relying
// on inherited page centring, which this codebase is phasing out — see #235)
// so dialog content stays centred regardless of what text-align the page
// behind it happens to use.
export function Dialog(props: {
  onDismiss?: () => void;
  width?: string;
  className?: string;
  children: ReactNode;
}) {
  return createPortal(
    <div className="fixed inset-0 z-10 flex items-center justify-center">
      <div
        className="absolute inset-0 animate-fade-in bg-black/50 motion-reduce:animate-none"
        onClick={props.onDismiss}
      ></div>
      <div
        className={cn(
          "relative z-10 flex max-w-[92vw] animate-dialog-in flex-col gap-4 rounded-xl bg-surface p-4 text-center shadow-2xl motion-reduce:animate-none sm:p-6",
          props.width ?? "w-150",
          props.className,
        )}
      >
        {props.children}
      </div>
    </div>,
    document.body,
  );
}

export function DialogTitle({ children }: { children: ReactNode }) {
  return <h2 className="m-0 text-2xl font-bold">{children}</h2>;
}

// Right-aligned row of buttons, normally the last child of a Dialog.
export function DialogActions({ children }: { children: ReactNode }) {
  return <div className="flex justify-end gap-3">{children}</div>;
}

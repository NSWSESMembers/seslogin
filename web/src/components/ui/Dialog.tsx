import type { ReactNode } from "react";
import { createPortal } from "react-dom";

// Centred modal dialog over a dimmed backdrop, both fading in on mount.
// Clicking the backdrop calls onDismiss (omit it to make the dialog
// non-dismissable). `width` overrides the default panel width — pass a width
// utility rather than putting one in `className`, since two width classes in
// one string resolve by stylesheet order, not prop order.
//
// Portalled to <body> so the panel inherits page-level typography (the global
// `body` centring, normal `white-space`) regardless of where it's rendered
// from — a Dialog opened from inside e.g. a right-aligned, `whitespace-nowrap`
// table cell would otherwise inherit those and mangle its own contents.
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
        className={`relative z-10 flex ${props.width ?? "w-150"} max-w-[92vw] animate-dialog-in flex-col gap-4 rounded-xl bg-surface p-4 shadow-2xl motion-reduce:animate-none sm:p-6 ${props.className ?? ""}`}
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

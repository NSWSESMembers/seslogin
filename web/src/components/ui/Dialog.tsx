import type { ReactNode } from "react";

// Centred modal dialog over a dimmed backdrop, both fading in on mount.
// Clicking the backdrop calls onDismiss (omit it to make the dialog
// non-dismissable). `width` overrides the default panel width — pass a width
// utility rather than putting one in `className`, since two width classes in
// one string resolve by stylesheet order, not prop order.
export function Dialog(props: {
  onDismiss?: () => void;
  width?: string;
  className?: string;
  children: ReactNode;
}) {
  return (
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
    </div>
  );
}

export function DialogTitle({ children }: { children: ReactNode }) {
  return <h2 className="m-0 text-2xl font-bold">{children}</h2>;
}

// Right-aligned row of buttons, normally the last child of a Dialog.
export function DialogActions({ children }: { children: ReactNode }) {
  return <div className="flex justify-end gap-3">{children}</div>;
}

import type { ReactNode } from "react";

// Centred modal dialog over a dimmed backdrop. Clicking the backdrop calls
// onDismiss (omit it to make the dialog non-dismissable).
export function Dialog(props: {
  onDismiss?: () => void;
  className?: string;
  children: ReactNode;
}) {
  return (
    <div className="fixed inset-0 z-10 flex items-center justify-center">
      <div
        className="absolute inset-0 bg-black opacity-50"
        onClick={props.onDismiss}
      ></div>
      <div
        className={`relative z-10 flex w-150 max-w-[90vw] flex-col gap-4 rounded-xl bg-surface p-6 shadow-2xl ${props.className ?? ""}`}
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

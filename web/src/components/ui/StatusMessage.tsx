import type { ReactNode } from "react";
import { cn } from "../../lib/tw";

const variantClasses = {
  error: "text-red-600",
  warning: "text-orange-600",
  success: "text-green-700",
} as const;

export type StatusMessageVariant = keyof typeof variantClasses;

// Lighter, non-boxed sibling of PanelMessage (components/ui/Panel.tsx) — an
// inline status line under a form field or table, not a bordered panel.
export function StatusMessage({
  variant,
  className,
  children,
}: {
  variant: StatusMessageVariant;
  className?: string;
  children: ReactNode;
}) {
  return (
    <p
      className={cn(
        "font-bold whitespace-pre-line",
        variantClasses[variant],
        className,
      )}
    >
      {children}
    </p>
  );
}

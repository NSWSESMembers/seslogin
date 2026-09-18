import type { ThHTMLAttributes, TdHTMLAttributes, ReactNode } from "react";
import { cn } from "../../lib/tw";

export function AdminTable({ children }: { children: ReactNode }) {
  return (
    <div className="overflow-x-auto">
      <table className="w-full border-collapse text-left">{children}</table>
    </div>
  );
}

type ThProps = ThHTMLAttributes<HTMLTableCellElement> & {
  section?: boolean;
};

export function Th({ section, className, ...props }: ThProps) {
  const base = section
    ? "border-b border-line px-2 pt-5 pb-1.5 font-title text-navy"
    : "border-b-2 border-line px-2 py-1.5 text-sm font-semibold text-ink-strong";
  return <th className={cn(base, className)} {...props} />;
}

type TdProps = TdHTMLAttributes<HTMLTableCellElement> & {
  nowrap?: boolean;
  center?: boolean;
  options?: boolean;
};

export function Td({ nowrap, center, options, className, ...props }: TdProps) {
  const classes = options
    ? "w-px border-b border-line-faint p-1 text-right whitespace-nowrap"
    : cn(
        "border-b border-line-faint px-2 py-1.5",
        nowrap && "whitespace-nowrap",
        center && "text-center align-middle",
      );
  return <td className={cn(classes, className)} {...props} />;
}

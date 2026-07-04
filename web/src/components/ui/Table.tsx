import type { ThHTMLAttributes, TdHTMLAttributes, ReactNode } from "react";

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
    ? "border-b border-neutral-300 px-2 pt-5 pb-1.5 font-title text-navy"
    : "border-b-2 border-neutral-300 px-2 py-1.5 text-sm font-semibold text-neutral-900";
  return (
    <th className={[base, className].filter(Boolean).join(" ")} {...props} />
  );
}

type TdProps = TdHTMLAttributes<HTMLTableCellElement> & {
  nowrap?: boolean;
  center?: boolean;
  options?: boolean;
};

export function Td({ nowrap, center, options, className, ...props }: TdProps) {
  const classes = options
    ? "w-px whitespace-nowrap border-b border-neutral-100 px-1 py-1 text-right"
    : [
        "border-b border-neutral-100 px-2 py-1.5",
        nowrap && "whitespace-nowrap",
        center && "text-center align-middle",
      ]
        .filter(Boolean)
        .join(" ");
  return (
    <td className={[classes, className].filter(Boolean).join(" ")} {...props} />
  );
}

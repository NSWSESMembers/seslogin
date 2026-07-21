import type { ReactNode } from "react";

interface OptionListProps {
  children: ReactNode;
  role?: string;
  "aria-label"?: string;
}

export function OptionList({ children, ...groupProps }: OptionListProps) {
  return (
    <div className="grid gap-2" {...groupProps}>
      {children}
    </div>
  );
}

interface OptionRowProps {
  input: ReactNode;
  title: ReactNode;
  description: ReactNode;
}

export function OptionRow({ input, title, description }: OptionRowProps) {
  return (
    <label className="flex cursor-pointer items-start gap-3 rounded-md border border-line p-3 transition-colors hover:border-menu hover:bg-brand/5 has-checked:border-menu has-checked:bg-brand/5">
      {input}
      <span className="flex flex-col gap-0.5">
        <span className="font-semibold text-ink">{title}</span>
        <span className="text-ink-muted">{description}</span>
      </span>
    </label>
  );
}

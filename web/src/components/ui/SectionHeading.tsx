import type { HTMLAttributes } from "react";
import { cn } from "../../lib/tw";

// A plain sub-heading within a page — an admin Settings section, a table
// caption. Preflight zeroes a bare <h2>'s font-size/weight, so without this a
// bare <h2> renders at body size and normal weight, indistinguishable from
// the text around it. Sits below DialogTitle (components/ui/Dialog.tsx) and
// PanelTitle (components/ui/Panel.tsx) in weight — those open a modal or a
// full page, this labels a section within one.
export function SectionHeading({
  className,
  ...props
}: HTMLAttributes<HTMLHeadingElement>) {
  return (
    <h2 className={cn("mb-2 text-lg font-semibold", className)} {...props} />
  );
}

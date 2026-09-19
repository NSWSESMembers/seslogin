import type { HTMLAttributes } from "react";
import { cn } from "../../lib/tw";

// Secondary/helper text. No default margin — callers that need space around
// it say so explicitly via `className` (or rely on a flex/gap parent), rather
// than the amount varying by which of several ad hoc spacing utilities the
// author who wrote that call site happened to reach for.
export function Muted({
  className,
  ...props
}: HTMLAttributes<HTMLParagraphElement>) {
  return <p className={cn("text-ink-muted", className)} {...props} />;
}

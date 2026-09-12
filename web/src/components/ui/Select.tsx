import type { SelectHTMLAttributes } from "react";
import { inputBase, inputWidths, type InputWidth } from "./inputStyles";

type SelectProps = SelectHTMLAttributes<HTMLSelectElement> & {
  width?: InputWidth;
};

export default function Select({
  width = "full",
  className,
  children,
  ...props
}: SelectProps) {
  return (
    <select
      className={[inputBase, "h-7.5 text-sm", inputWidths[width], className]
        .filter(Boolean)
        .join(" ")}
      {...props}
    >
      {children}
    </select>
  );
}

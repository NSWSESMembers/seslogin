import type { SelectHTMLAttributes } from "react";
import { inputBase, type InputWidth } from "./inputStyles";
import { tw } from "../../lib/tw";

const widths: Record<InputWidth, string> = {
  full: tw`w-full md:w-[92%]`,
  medium: tw`w-full md:w-[70%]`,
  half: tw`w-full md:w-[45%]`,
  small: tw`w-full md:w-[25%]`,
  auto: tw`w-auto`,
};

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
      className={[inputBase, "h-7.5 text-sm", widths[width], className]
        .filter(Boolean)
        .join(" ")}
      {...props}
    >
      {children}
    </select>
  );
}

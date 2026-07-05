import type { InputHTMLAttributes } from "react";
import { tw } from "../../lib/tw";
import { inputBase, type InputWidth } from "./inputStyles";

const widths: Record<InputWidth, string> = {
  full: tw`w-full md:w-[90%]`,
  medium: tw`w-full md:w-[70%]`,
  half: tw`w-full md:w-[45%]`,
  small: tw`w-full md:w-[25%]`,
  auto: tw`w-auto`,
};

type TextInputProps = InputHTMLAttributes<HTMLInputElement> & {
  width?: InputWidth;
};

export default function TextInput({
  width = "full",
  className,
  ...props
}: TextInputProps) {
  return (
    <input
      className={[inputBase, "text-sm", widths[width], className]
        .filter(Boolean)
        .join(" ")}
      {...props}
    />
  );
}

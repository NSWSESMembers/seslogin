import { tw } from "../../lib/tw";

export type InputWidth = "full" | "medium" | "half" | "small" | "auto";

export const inputBase = tw`rounded-md border border-line bg-surface px-2 py-1 transition-colors focus:border-menu focus:ring-2 focus:ring-menu/25 focus:outline-none`;

/** Shared by `Select` and `Combobox`, which must line up in the same form. */
export const inputWidths: Record<InputWidth, string> = {
  full: tw`w-full md:w-[92%]`,
  medium: tw`w-full md:w-[70%]`,
  half: tw`w-full md:w-[45%]`,
  small: tw`w-full md:w-[25%]`,
  auto: tw`w-auto`,
};

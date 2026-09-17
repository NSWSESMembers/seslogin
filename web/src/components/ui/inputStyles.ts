import { tw } from "../../lib/tw";

export type InputWidth = "full" | "medium" | "half" | "small" | "auto";

export const inputBase = tw`rounded-md border border-line bg-surface px-2 py-1 transition-colors focus:border-menu focus:ring-2 focus:ring-menu/25 focus:outline-none`;

/**
 * `inputBase`'s focus ring, for a control whose focusable element sits
 * *inside* a bordered wrapper (`MultiCombobox`'s token box). The `focus:`
 * variants in `inputBase` never fire on the wrapper `<div>` itself, which
 * isn't focusable — only the bare input nested inside it is.
 */
export const inputFocusWithin = tw`focus-within:border-menu focus-within:ring-2 focus-within:ring-menu/25 focus-within:outline-none`;

/** Shared by `Select` and `Combobox`, which must line up in the same form. */
export const inputWidths: Record<InputWidth, string> = {
  full: tw`w-full md:w-[92%]`,
  medium: tw`w-full md:w-[70%]`,
  half: tw`w-full md:w-[45%]`,
  small: tw`w-full md:w-[25%]`,
  auto: tw`w-auto`,
};

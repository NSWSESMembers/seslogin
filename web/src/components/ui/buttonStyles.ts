import { tw } from "../../lib/tw";

export type ButtonVariant = "primary" | "danger" | "secondary" | "kiosk";
export type ButtonSize = "normal" | "row" | "panel" | "bare";

export const buttonVariants: Record<ButtonVariant, string> = {
  primary: tw`bg-neutral-800 text-white transition-colors hover:bg-neutral-700 active:bg-neutral-900 disabled:cursor-wait disabled:opacity-60`,
  danger: tw`bg-red-700 text-white transition-colors hover:bg-red-600 disabled:cursor-wait disabled:opacity-60`,
  secondary: tw`bg-slate-500 text-white transition-colors hover:bg-slate-400 disabled:cursor-wait disabled:opacity-60`,
  kiosk: tw`bg-neutral-800 font-title text-white active:bg-neutral-600 disabled:cursor-default disabled:opacity-30`,
};

export const buttonSizes: Record<ButtonSize, string> = {
  normal: tw`rounded-md px-4 py-1.5 text-sm font-medium`,
  row: tw`rounded px-2 py-0.5 text-xs font-medium`,
  panel: tw`min-w-62.5 rounded-md px-6 py-3 text-lg`,
  bare: tw`rounded-lg`,
};

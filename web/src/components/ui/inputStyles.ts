import { tw } from "../../lib/tw";

export type InputWidth = "full" | "medium" | "half" | "small";

export const inputBase = tw`rounded-md border border-neutral-300 bg-white px-2 py-1 transition-colors focus:border-menu focus:ring-2 focus:ring-menu/25 focus:outline-none`;

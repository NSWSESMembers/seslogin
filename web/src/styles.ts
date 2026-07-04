import { tw } from "./lib/tw";

export type ScreenPosition = "offLeft" | "center" | "offRight";

export const scanView = tw`absolute w-full px-2.5 transition-[left] duration-500 ease-in-out`;

export const scanViewPosition: Record<ScreenPosition, string> = {
  offLeft: tw`-left-full`,
  center: tw`left-0`,
  offRight: tw`left-full`,
};

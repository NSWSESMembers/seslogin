import { tw } from "./lib/tw";

export type ScreenPosition = "offLeft" | "center" | "offRight";

export const scanView = tw`absolute left-0 w-full px-2.5 transition-transform duration-500 ease-in-out`;

export const scanViewPosition: Record<ScreenPosition, string> = {
  offLeft: tw`-translate-x-full`,
  center: tw`translate-x-0`,
  offRight: tw`translate-x-full`,
};

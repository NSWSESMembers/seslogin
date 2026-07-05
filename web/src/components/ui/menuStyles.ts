import { tw } from "../../lib/tw";

export type MenuLevel = "main" | "sub";

export const menuLevelClasses: Record<MenuLevel, string> = {
  main: tw`px-2.5 py-0.25 text-lg text-white no-underline hover:bg-menu-hover aria-[current=page]:bg-menu-active`,
  sub: tw`px-2.5 py-0.25 text-sm text-white no-underline hover:bg-submenu-hover aria-[current=page]:bg-submenu-active`,
};

export const menuButtonClasses = tw`cursor-pointer border-0 bg-transparent px-2.5 py-0.25 font-[inherit] text-lg text-white no-underline hover:bg-menu-hover`;

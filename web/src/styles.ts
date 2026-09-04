import { tw } from "./lib/tw";

export type ScreenPosition = "offLeft" | "center" | "offRight";

export const scanView = tw`absolute left-0 w-full px-2.5 transition-transform duration-500 ease-in-out`;

export const scanViewPosition: Record<ScreenPosition, string> = {
  offLeft: tw`-translate-x-full`,
  center: tw`translate-x-0`,
  offRight: tw`translate-x-full`,
};

/**
 * The className and `inert` a kiosk screen's outer div needs, given where it
 * currently sits.
 *
 * Every scan screen stays mounted at all times so it can animate in and out, so
 * at any moment four of the five are parked off-side but still in the DOM —
 * and, without `inert`, still in the tab order. On the sign-out adjust screen
 * that is 32 reachable buttons where 6 are on screen: Tab walks the operator
 * into four screens they cannot see. `inert` takes an off-screen screen out of
 * the focus order and out of hit-testing until it slides back, which cuts that
 * to the 6 that are actually visible.
 *
 * It also makes structural a rule the scan screen currently keeps by hand: the
 * main screen holds focus in its member-ID input for barcode scans, and
 * `scanFocusLeases` exists to stop it grabbing focus back while another screen
 * is up. An inert screen cannot take focus at all, so that can no longer race.
 *
 * It is returned as one object rather than left to each screen so that a screen
 * added later cannot quietly omit it.
 */
export function scanViewProps(
  position: ScreenPosition,
  extraClasses = "",
): { className: string; inert: boolean } {
  const extra = extraClasses ? ` ${extraClasses}` : "";
  return {
    className: `${scanView} ${scanViewPosition[position]}${extra}`,
    inert: position !== "center",
  };
}

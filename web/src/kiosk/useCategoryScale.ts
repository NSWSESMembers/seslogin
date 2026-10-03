import { useLayoutEffect, useState, type RefObject } from "react";
import { categories } from "../lib/categories";
import { fitCategoryScale, longestCategoryList } from "./categoryScale";
import { CATEGORY_BORDER_PX } from "./categoryScale";

const ITEM_COUNT = longestCategoryList(categories);

function editableFocused(): boolean {
  const el = document.activeElement;
  return (
    el instanceof HTMLInputElement ||
    el instanceof HTMLTextAreaElement ||
    (el instanceof HTMLElement && el.isContentEditable)
  );
}

/**
 * The scale (see categoryScale.ts) for the category buttons on this kiosk.
 *
 * Decided once for the whole kiosk, always against the *longest* category
 * list rather than whichever one is showing, so the buttons never change size
 * between the top level and a sub-list, or mid slide transition, and the result
 * can't oscillate with what's rendered.
 *
 * `hostRef` is the scan area (the space under the title bar); `probeRef` is a
 * `CategoryScaleProbe` inside it. The first measurement happens in a layout
 * effect, before first paint, so there is no large-to-small jump on load.
 *
 * On-screen keyboard: the page is `h-dvh` and the main screen keeps focus in
 * the member-ID input, so on a tablet the software keyboard shrinks the host's
 * height. Shrinking the buttons for that would make them jump every time the
 * keyboard opens, so a height *decrease* at an unchanged width is ignored while
 * an input is focused. Anything else (a width change from rotation or a
 * window resize, a height increase such as the keyboard closing, or a decrease
 * with nothing focused) recomputes normally. Known limit: resizing a desktop
 * browser window shorter while the member-ID input has focus is ignored until
 * the next width change or reload; kiosks are fullscreen, so that is not a case
 * worth a heuristic.
 */
export function useCategoryScale(
  hostRef: RefObject<HTMLElement | null>,
  probeRef: RefObject<HTMLElement | null>,
): number {
  const [scale, setScale] = useState(1);

  useLayoutEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let accepted: { width: number; height: number } | null = null;

    function measureAndFit() {
      const probe = probeRef.current;
      if (!host || !probe) return;
      const li = probe.querySelector("ul > li");
      const grid = probe.querySelector('[data-probe="grid"]');
      const header = probe.querySelector('[data-probe="header"]');
      if (!li || !grid || !header) return;

      const width = host.clientWidth;
      const height = host.clientHeight;
      if (
        accepted &&
        width === accepted.width &&
        height < accepted.height &&
        editableFocused()
      ) {
        return;
      }
      accepted = { width, height };

      const cell = li.getBoundingClientRect();
      const headerRect = header.getBoundingClientRect();
      const headerMarginTop = parseFloat(getComputedStyle(header).marginTop);
      setScale(
        fitCategoryScale({
          width: grid.getBoundingClientRect().width,
          height,
          itemCount: ITEM_COUNT,
          cell: {
            width: cell.width,
            height: cell.height,
            fixedWidth: CATEGORY_BORDER_PX * 2,
            fixedHeight: CATEGORY_BORDER_PX * 2,
          },
          headerHeight:
            headerRect.height +
            (Number.isNaN(headerMarginTop) ? 0 : headerMarginTop),
        }),
      );
    }

    measureAndFit();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measureAndFit);
    observer.observe(host);
    return () => observer.disconnect();
  }, [hostRef, probeRef]);

  return scale;
}

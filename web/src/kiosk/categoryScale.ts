import type { Category } from "../lib/categories";

/**
 * Category buttons size themselves to the kiosk screen. One scale factor `s`
 * is picked per kiosk and exposed as the `--cat-scale` CSS custom property on
 * the scan area; the buttons derive their size, padding, margin, font and icon
 * from it (`calc(<px at s=1> * var(--cat-scale, 1))`), so s=1 is the original
 * "large" button.
 *
 * `s` is the largest value in [MIN_CATEGORY_SCALE, 1] (in CATEGORY_SCALE_STEP
 * steps) at which the *longest* category list still fits the screen without
 * scrolling. Below the minimum the buttons stop shrinking and the screen
 * scrolls instead.
 *
 * MIN_CATEGORY_SCALE is 0.65: a button is then ~98x75px with ~12px text and a
 * 45px icon, a little under the old fixed "small" size (about 0.75) and still
 * an easy tap target. Smaller than that the text stops being comfortably
 * readable from arm's length, which matters more than fitting a 27-item list on
 * a very small screen.
 */
export const MIN_CATEGORY_SCALE = 0.65;
export const CATEGORY_SCALE_STEP = 0.05;

/** The border does not scale (a fractional border is snapped by the browser). */
export const CATEGORY_BORDER_PX = 2;

/** Size of a button's cell (its margin box) at s=1, as measured in the DOM. */
export type CellMetrics = {
  width: number;
  height: number;
  /** The part of width/height that does not scale with s (the borders). */
  fixedWidth?: number;
  fixedHeight?: number;
};

export type FitInput = {
  /** Width the button grid can use (inside the screen's side padding). */
  width: number;
  /** Height of the scan area, i.e. under the title bar. */
  height: number;
  /** Length of the longest list that must fit. */
  itemCount: number;
  cell: CellMetrics;
  /** Height of the list's header (title row), which does not scale. */
  headerHeight: number;
  min?: number;
  step?: number;
};

function cellSize(cell: CellMetrics, scale: number) {
  const fw = cell.fixedWidth ?? 0;
  const fh = cell.fixedHeight ?? 0;
  return {
    width: fw + (cell.width - fw) * scale,
    height: fh + (cell.height - fh) * scale,
  };
}

/** Height a list of `itemCount` buttons plus its header takes at `scale`. */
export function neededHeight(
  input: Omit<FitInput, "min" | "step">,
  scale: number,
): number {
  const { width, itemCount, cell, headerHeight } = input;
  const size = cellSize(cell, scale);
  const columns = Math.max(1, Math.floor(width / size.width));
  const rows = Math.ceil(itemCount / columns);
  return headerHeight + rows * size.height;
}

/**
 * The largest scale in [min, 1] at which `itemCount` buttons plus the header
 * fit in `height`; `min` when none does. Pure (no DOM) so it can be unit tested.
 */
export function fitCategoryScale(input: FitInput): number {
  const min = input.min ?? MIN_CATEGORY_SCALE;
  const step = input.step ?? CATEGORY_SCALE_STEP;
  const { width, height, cell } = input;
  // No layout (jsdom, or a hidden host): there is nothing to fit against.
  if (!(width > 0 && height > 0 && cell.width > 0 && cell.height > 0)) {
    return 1;
  }
  const steps = Math.floor((1 - min) / step + 1e-9);
  for (let i = 0; i <= steps; i++) {
    const scale = Math.round((1 - i * step) * 1000) / 1000;
    if (neededHeight(input, scale) <= height) {
      return scale;
    }
  }
  return min;
}

/** Length of the longest list the category screen can show. */
export function longestCategoryList(categories: Category[]): number {
  return Math.max(
    categories.length,
    ...categories.map((c) => c.subcategories?.length ?? 0),
  );
}

import { describe, expect, it } from "vitest";
import { categories } from "../lib/categories";
import {
  CATEGORY_BORDER_PX,
  MIN_CATEGORY_SCALE,
  fitCategoryScale,
  longestCategoryList,
  neededHeight,
} from "./categoryScale";

// Measured at s=1 on the real scan screen: a button's margin box (the border
// is the part that does not scale) and the drill-down header (mt-5 + the
// 52px back button).
const cell = {
  width: 198,
  height: 166,
  fixedWidth: CATEGORY_BORDER_PX * 2,
  fixedHeight: CATEGORY_BORDER_PX * 2,
};
const headerHeight = 72;
// Scan screens have 10px of side padding each.
const grid = (screenWidth: number) => screenWidth - 20;

const TITLE_BAR = 88;
function fit(screenWidth: number, screenHeight: number, itemCount = 27) {
  return fitCategoryScale({
    width: grid(screenWidth),
    height: screenHeight - TITLE_BAR,
    itemCount,
    cell,
    headerHeight,
  });
}

describe("longestCategoryList", () => {
  it("is the real tree's biggest list", () => {
    expect(longestCategoryList(categories)).toBe(27);
  });
  it("counts the top level too", () => {
    expect(
      longestCategoryList([
        { id: "a", name: "a", icon: "", subcategories: [] },
        { id: "b", name: "b", icon: "", subcategories: [] },
      ]),
    ).toBe(2);
  });
});

describe("fitCategoryScale", () => {
  it("is full size when the list already fits", () => {
    expect(fit(1920, 1080)).toBe(1);
    expect(
      fitCategoryScale({
        width: 1900,
        height: 1000,
        itemCount: 5,
        cell,
        headerHeight,
      }),
    ).toBe(1);
  });

  it("shrinks a long list on a mid-sized screen", () => {
    const s = fit(1280, 800);
    expect(s).toBeLessThan(1);
    expect(s).toBeGreaterThan(MIN_CATEGORY_SCALE);
  });

  it("is the largest step that fits, and the next step up does not", () => {
    for (const [w, h] of [
      [1280, 800],
      [1366, 768],
      [1024, 768],
      [1024, 600],
    ]) {
      const s = fit(w, h);
      const input = {
        width: grid(w),
        itemCount: 27,
        cell,
        headerHeight,
        height: h - TITLE_BAR,
      };
      if (s > MIN_CATEGORY_SCALE) {
        expect(neededHeight(input, s)).toBeLessThanOrEqual(input.height);
      }
      if (s < 1) {
        expect(neededHeight(input, s + 0.05)).toBeGreaterThan(input.height);
      }
    }
  });

  it("stops at the minimum on screens too small to fit", () => {
    expect(fit(800, 480)).toBe(MIN_CATEGORY_SCALE);
  });

  it("never exceeds 1 or goes below the minimum", () => {
    for (let w = 400; w <= 2400; w += 200) {
      for (let h = 300; h <= 1400; h += 100) {
        const s = fit(w, h);
        expect(s).toBeLessThanOrEqual(1);
        expect(s).toBeGreaterThanOrEqual(MIN_CATEGORY_SCALE);
      }
    }
  });

  it("is monotonic in the available height", () => {
    let last = 0;
    for (let h = 400; h <= 1400; h += 25) {
      const s = fit(1366, h);
      expect(s).toBeGreaterThanOrEqual(last);
      last = s;
    }
  });

  it("grows with a wider screen (more columns, fewer rows)", () => {
    expect(fit(1920, 768)).toBeGreaterThan(fit(1024, 768));
  });

  it("does not scale the border", () => {
    // At s=0.5 a 198px cell with a 4px fixed border is 4 + 194/2 = 101.
    const input = {
      width: 1010,
      height: 1000,
      itemCount: 20,
      cell,
      headerHeight: 0,
    };
    // 1010/101 = exactly 10 columns -> 2 rows of (4 + 162/2 = 85) = 170.
    expect(neededHeight(input, 0.5)).toBeCloseTo(170, 6);
  });

  it("returns 1 when there is no layout to fit against", () => {
    expect(fit(0, 0)).toBe(1);
    expect(
      fitCategoryScale({
        width: 500,
        height: 500,
        itemCount: 5,
        cell: { width: 0, height: 0 },
        headerHeight: 0,
      }),
    ).toBe(1);
  });

  it("respects a custom min and step", () => {
    const s = fitCategoryScale({
      width: grid(800),
      height: 200,
      itemCount: 27,
      cell,
      headerHeight,
      min: 0.5,
      step: 0.1,
    });
    expect(s).toBe(0.5);
  });
});

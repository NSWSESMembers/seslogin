import { render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import Scan from "./Scan";
import { categories } from "../../lib/categories";
import { fitCategoryScale, longestCategoryList } from "../categoryScale";

const session = vi.hoisted(() => ({
  config: {} as { categorySearch?: boolean },
}));
vi.mock("../components/useKioskSession", () => ({
  useKioskSession: () => session,
}));
vi.mock("../components/ScanController", () => ({ default: () => null }));
vi.mock("../components/ScanTitleBar", () => ({ default: () => null }));
vi.mock("../components/KioskStatusDialog", () => ({ default: () => null }));
vi.mock("../../components/ClientVersionLabel", () => ({ default: () => null }));

// Keep the real probe and scaling hook together; jsdom supplies no layout, so
// give their rendered elements the same measurements as a kiosk screen.
function measureScreen() {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(
    function (this: HTMLElement) {
      const kind = this.getAttribute("data-probe");
      return {
        width: this.tagName === "LI" ? 198 : 800,
        height:
          this.tagName === "LI"
            ? 166
            : kind === "header"
              ? 72
              : kind === "search"
                ? 150
                : 0,
      } as DOMRect;
    },
  );
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(820);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(900);
}

function expectedScale(searchHeight: number) {
  return String(
    fitCategoryScale({
      width: 800,
      height: 900,
      itemCount: longestCategoryList(categories),
      cell: { width: 198, height: 166, fixedWidth: 4, fixedHeight: 4 },
      headerHeight: 72 + searchHeight,
    }),
  );
}

afterEach(() => {
  vi.restoreAllMocks();
  session.config = {};
});

describe("Scan", () => {
  it("fits the buttons around the search box when the live session setting changes", () => {
    measureScreen();
    const { container, rerender } = render(<Scan />);
    const host = container.querySelector<HTMLElement>(".overflow-hidden")!;
    expect(container.querySelector('[data-probe="search"]')).toBeNull();
    expect(host.style.getPropertyValue("--cat-scale")).toBe(expectedScale(0));

    session.config = { categorySearch: true };
    rerender(<Scan />);
    const search = container.querySelector('[data-probe="search"]');
    expect(search?.querySelector('input[role="searchbox"]')).not.toBeNull();
    expect(search?.querySelector('[role="group"]')).toBeNull();
    expect(host.style.getPropertyValue("--cat-scale")).toBe(expectedScale(150));

    session.config = { categorySearch: false };
    rerender(<Scan />);
    expect(container.querySelector('[data-probe="search"]')).toBeNull();
    expect(host.style.getPropertyValue("--cat-scale")).toBe(expectedScale(0));
  });
});

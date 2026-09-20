import { describe, expect, it } from "vitest";
import { cn } from "./tw";

describe("cn", () => {
  it("resolves two conflicting paddings by keeping the later one", () => {
    expect(cn("p-2", "p-4")).toBe("p-4");
  });

  it("resolves two conflicting widths by keeping the later one", () => {
    expect(cn("w-32", "w-64")).toBe("w-64");
  });

  it("resolves a custom @theme color token against a stock Tailwind color", () => {
    expect(cn("text-navy", "text-red-600")).toBe("text-red-600");
    expect(cn("text-red-600", "text-navy")).toBe("text-navy");
    expect(cn("bg-danger-env", "bg-red-600")).toBe("bg-red-600");
    expect(cn("text-ink-muted", "text-gray-500")).toBe("text-gray-500");
  });

  it("resolves conflicting fractional custom spacing values", () => {
    expect(cn("min-w-62.5", "min-w-40")).toBe("min-w-40");
    expect(cn("max-w-136", "max-w-4xl")).toBe("max-w-4xl");
  });

  it("keeps non-conflicting classes from both arguments", () => {
    expect(cn("px-4 text-sm", "font-bold")).toBe("px-4 text-sm font-bold");
  });

  it("drops falsy values", () => {
    expect(cn("px-4", false, null, undefined, "text-sm")).toBe("px-4 text-sm");
  });
});

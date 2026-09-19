import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Muted } from "./Muted";

describe("Muted", () => {
  it("renders the muted text color with no default margin", () => {
    render(<Muted>Secondary text</Muted>);
    const el = screen.getByText("Secondary text");
    expect(el).toHaveClass("text-ink-muted");
    expect(el).not.toHaveClass("m-0");
    expect(el.className).not.toMatch(/\bm[tby]?-/);
  });

  it("merges a caller className instead of dropping the muted color", () => {
    render(<Muted className="mb-8 text-center">Secondary text</Muted>);
    const el = screen.getByText("Secondary text");
    expect(el).toHaveClass("text-ink-muted");
    expect(el).toHaveClass("mb-8");
    expect(el).toHaveClass("text-center");
  });
});

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Dialog } from "./Dialog";

describe("Dialog", () => {
  // jsdom has no layout, so this pins the classes rather than real overflow:
  // a panel taller than the screen must scroll instead of running off it.
  it("caps the panel at the viewport height and lets it scroll", () => {
    render(
      <Dialog>
        <p>Body</p>
      </Dialog>,
    );
    const panel = screen.getByText("Body").parentElement!;
    expect(panel).toHaveClass("max-h-[calc(100dvh-1rem)]");
    expect(panel).toHaveClass("overflow-y-auto");
  });
});

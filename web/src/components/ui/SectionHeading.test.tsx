import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { SectionHeading } from "./SectionHeading";

describe("SectionHeading", () => {
  it("renders as an h2 with an explicit size and weight", () => {
    render(<SectionHeading>Passkeys</SectionHeading>);
    const heading = screen.getByRole("heading", { level: 2, name: "Passkeys" });
    expect(heading).toHaveClass("text-lg");
    expect(heading).toHaveClass("font-semibold");
  });

  it("merges a caller className instead of dropping its own classes", () => {
    render(<SectionHeading className="mt-8">Kiosk diagnostics</SectionHeading>);
    const heading = screen.getByRole("heading", { level: 2 });
    expect(heading).toHaveClass("mt-8");
    expect(heading).toHaveClass("font-semibold");
  });
});

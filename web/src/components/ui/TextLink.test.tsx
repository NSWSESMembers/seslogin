import "@testing-library/jest-dom/vitest";
import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { TextLink } from "./TextLink";

describe("TextLink", () => {
  it("renders a route with `to` and anything else with `href`, both styled", () => {
    const screen = render(
      <MemoryRouter>
        <TextLink to="/guide">Guide</TextLink>
        <TextLink href="mailto:support@seslogin.com" className="font-bold">
          Email
        </TextLink>
      </MemoryRouter>,
    );

    const route = screen.getByRole("link", { name: "Guide" });
    expect(route).toHaveAttribute("href", "/guide");
    expect(route).toHaveClass("text-navy", "underline");

    const mail = screen.getByRole("link", { name: "Email" });
    expect(mail).toHaveAttribute("href", "mailto:support@seslogin.com");
    expect(mail).toHaveClass("text-navy", "underline", "font-bold");
  });
});

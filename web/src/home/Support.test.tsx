import "@testing-library/jest-dom/vitest";
import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import Support from "./Support";

describe("Support", () => {
  it("gives the support email address without signing in", () => {
    const screen = render(
      <MemoryRouter>
        <Support />
      </MemoryRouter>,
    );

    expect(
      screen.getByRole("heading", { name: "Support" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "support@seslogin.com" }),
    ).toHaveAttribute("href", "mailto:support@seslogin.com");
    expect(
      screen.getByRole("link", { name: "Write an email" }),
    ).toHaveAttribute("href", "mailto:support@seslogin.com");
    expect(
      screen.getByRole("link", { name: "SESLogin user guides" }),
    ).toHaveAttribute("href", "/guide");
  });
});

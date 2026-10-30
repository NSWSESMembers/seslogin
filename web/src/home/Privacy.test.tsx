import "@testing-library/jest-dom/vitest";
import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import Privacy from "./Privacy";

describe("Privacy", () => {
  it("covers what the app stores require of a privacy policy", () => {
    const screen = render(
      <MemoryRouter>
        <Privacy />
      </MemoryRouter>,
    );

    for (const name of [
      "What we collect",
      "How we use it",
      "Who can see it",
      "How long we keep it",
      "Your choices",
    ]) {
      expect(screen.getByRole("heading", { name })).toBeInTheDocument();
    }
    for (const link of screen.getAllByRole("link", {
      name: "support@seslogin.com",
    })) {
      expect(link).toHaveAttribute("href", "mailto:support@seslogin.com");
    }
  });
});

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { StatusMessage } from "./StatusMessage";

describe("StatusMessage", () => {
  it("renders each variant's color class", () => {
    const { rerender } = render(
      <StatusMessage variant="error">Oops</StatusMessage>,
    );
    expect(screen.getByText("Oops")).toHaveClass("text-red-600");

    rerender(<StatusMessage variant="warning">Careful</StatusMessage>);
    expect(screen.getByText("Careful")).toHaveClass("text-orange-600");

    rerender(<StatusMessage variant="success">Done</StatusMessage>);
    expect(screen.getByText("Done")).toHaveClass("text-green-700");
  });

  it("merges a caller className rather than dropping the variant color", () => {
    render(
      <StatusMessage variant="error" className="m-0">
        Oops
      </StatusMessage>,
    );
    const el = screen.getByText("Oops");
    expect(el).toHaveClass("m-0");
    expect(el).toHaveClass("text-red-600");
  });

  it("preserves literal newlines via whitespace-pre-line", () => {
    render(
      <StatusMessage variant="warning">{"line one\nline two"}</StatusMessage>,
    );
    expect(screen.getByText(/line one/)).toHaveClass("whitespace-pre-line");
  });
});

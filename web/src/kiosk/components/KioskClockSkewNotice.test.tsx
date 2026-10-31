import "@testing-library/jest-dom/vitest";
import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import KioskClockSkewNotice from "./KioskClockSkewNotice";
import { clearClockSkew, reportClockSkew } from "../lib/clockSkew";

beforeEach(() => {
  vi.spyOn(console, "warn").mockImplementation(() => {});
});

afterEach(() => {
  act(() => clearClockSkew());
  vi.restoreAllMocks();
});

describe("KioskClockSkewNotice", () => {
  it("shows nothing while the clock is fine", () => {
    render(<KioskClockSkewNotice />);
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("asks for the clock to be fixed, says how far out it is, and goes once it's right", () => {
    render(<KioskClockSkewNotice />);

    act(() => reportClockSkew(Math.floor(Date.now() / 1000) - 600));
    expect(screen.getByRole("alertdialog")).toHaveTextContent(
      "This computer's clock is wrong",
    );
    expect(screen.getByText("10 minutes fast")).toBeInTheDocument();
    expect(screen.getByRole("alertdialog")).toHaveTextContent(
      "no need to scan a QR code",
    );

    act(() => clearClockSkew());
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });
});

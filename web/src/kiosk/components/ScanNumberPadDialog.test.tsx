import { render, screen } from "@testing-library/react";
import { describe, expect, it, vitest } from "vitest";
import UserEvent from "@testing-library/user-event";
import ScanNumberPadDialog from "./ScanNumberPadDialog";

function renderPad(overrides?: { value?: string; submitDisabled?: boolean }) {
  const handlers = {
    onDigit: vitest.fn(),
    onDelete: vitest.fn(),
    onSubmit: vitest.fn(),
    onClose: vitest.fn(),
  };
  render(
    <ScanNumberPadDialog
      value={overrides?.value ?? ""}
      submitDisabled={overrides?.submitDisabled}
      {...handlers}
    />,
  );
  return handlers;
}

describe("ScanNumberPadDialog", () => {
  it("shows the typed digits in the Member ID display", () => {
    renderPad({ value: "123" });
    expect(screen.getByRole("status", { name: "Member ID" })).toHaveTextContent(
      "123",
    );
  });

  it("sends digits, delete, close and confirm from the keypad", async () => {
    const user = UserEvent.setup();
    const handlers = renderPad();
    await user.click(screen.getByRole("button", { name: "7" }));
    await user.click(screen.getByRole("button", { name: "0" }));
    expect(handlers.onDigit.mock.calls).toEqual([["7"], ["0"]]);
    await user.click(screen.getByRole("button", { name: "Delete" }));
    expect(handlers.onDelete).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "Confirm" }));
    expect(handlers.onSubmit).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(handlers.onClose).toHaveBeenCalledTimes(1);
  });

  it("disables Confirm, and Enter, while submitDisabled", async () => {
    const user = UserEvent.setup();
    const handlers = renderPad({ submitDisabled: true });
    expect(screen.getByRole("button", { name: "Confirm" })).toBeDisabled();
    await user.keyboard("{Enter}");
    expect(handlers.onSubmit).not.toHaveBeenCalled();
  });
});

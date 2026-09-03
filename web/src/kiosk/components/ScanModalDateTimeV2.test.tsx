import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vitest } from "vitest";
import UserEvent from "@testing-library/user-event";
import { Inner } from "./ScanModalDateTimeV2";
import { isSameDay } from "../../lib/time";

// The digit boxes are the only elements carrying the caret ring, so "which box
// has border-accent" is the assertion for where the caret is.
function digitBoxes(): HTMLElement[] {
  return [0, 1, 2, 3].map((i) =>
    screen.getByLabelText(`${i < 2 ? "Hour" : "Minute"} digit ${(i % 2) + 1}`),
  );
}

function digitText(): string {
  return digitBoxes()
    .map((box) => box.textContent!.replace("\xa0", "_"))
    .join("");
}

// There is always exactly one ring, so a -1 here is a failure, not a state.
function caretIndex(): number {
  const ringed = digitBoxes().filter((box) =>
    box.classList.contains("border-accent"),
  );
  expect(ringed).toHaveLength(1);
  return digitBoxes().indexOf(ringed[0]);
}

function renderInner(overrides?: {
  field?: string;
  initialValue?: string;
  onSave?: (field: string, date: Date, value: string) => void;
  onClose?: () => void;
}) {
  render(
    <Inner
      field={overrides?.field ?? "startTime"}
      initialDate={new Date(2026, 0, 15)}
      initialAmPm="AM"
      initialValue={overrides?.initialValue ?? "0930"}
      onSave={overrides?.onSave ?? vitest.fn()}
      onClose={overrides?.onClose ?? vitest.fn()}
    />,
  );
}

describe("ScanModalDateTimeV2", () => {
  // setToNow reads the wall clock, so the "Now" test freezes it; nothing in the
  // vitest config restores that, so put the real clock back by hand.
  afterEach(() => {
    vitest.useRealTimers();
  });

  it("opens with the caret on the first digit even when prefilled", () => {
    renderInner();
    expect(digitText()).toBe("0930");
    expect(caretIndex()).toBe(0);
  });

  it("puts the caret on the first blank digit", () => {
    renderInner({ initialValue: "09" });
    expect(digitText()).toBe("09__");
    expect(caretIndex()).toBe(2);
  });

  it("advances the caret right as digits are typed, wrapping at the end", async () => {
    const user = UserEvent.setup();
    renderInner({ initialValue: "" });
    expect(caretIndex()).toBe(0);
    await user.keyboard("1");
    expect(caretIndex()).toBe(1);
    await user.keyboard("4");
    expect(caretIndex()).toBe(2);
    await user.keyboard("3");
    expect(caretIndex()).toBe(3);
    await user.keyboard("5");
    expect(digitText()).toBe("1435");
    expect(caretIndex()).toBe(0);
  });

  it("parks the caret on a clicked digit and replaces it in place", async () => {
    const user = UserEvent.setup();
    renderInner({ initialValue: "0930" });
    await user.click(digitBoxes()[2]);
    expect(caretIndex()).toBe(2);
    await user.keyboard("4");
    expect(digitText()).toBe("0940");
    expect(caretIndex()).toBe(3);
  });

  it("rejects a digit that would make the time invalid, wherever the caret is", async () => {
    const user = UserEvent.setup();
    renderInner({ initialValue: "1930" });
    // hour units is 9, so the hour tens cannot become 2 (that would be 29:30)
    await user.click(digitBoxes()[0]);
    await user.keyboard("2");
    expect(digitText()).toBe("1930");
    // ...but 0 is fine
    await user.keyboard("0");
    expect(digitText()).toBe("0930");
    // minute tens tops out at 5
    await user.click(digitBoxes()[2]);
    await user.keyboard("6");
    expect(digitText()).toBe("0930");
  });

  it("never clears the digits after the one being entered", async () => {
    const user = UserEvent.setup();
    renderInner({ initialValue: "0930" });
    await user.keyboard("1");
    expect(digitText()).toBe("1930");
    expect(caretIndex()).toBe(1);
  });

  it("wraps to the first digit and keeps replacing in place", async () => {
    const user = UserEvent.setup();
    renderInner({ initialValue: "" });
    await user.keyboard("0930");
    expect(caretIndex()).toBe(0);
    await user.keyboard("1");
    expect(digitText()).toBe("1930");
    expect(caretIndex()).toBe(1);
  });

  it("backspaces the digit under the caret, else the one to its left", async () => {
    const user = UserEvent.setup();
    renderInner({ initialValue: "0930" });
    // caret on a filled digit: clears in place
    await user.keyboard("{Backspace}");
    expect(digitText()).toBe("_930");
    expect(caretIndex()).toBe(0);
    // caret on a blank with nothing to its left: no-op
    await user.keyboard("{Backspace}");
    expect(digitText()).toBe("_930");
    expect(caretIndex()).toBe(0);
    // caret on a blank: steps left and clears
    await user.click(digitBoxes()[2]);
    await user.keyboard("{Backspace}");
    expect(digitText()).toBe("_9_0");
    expect(caretIndex()).toBe(2);
    await user.keyboard("{Backspace}");
    expect(digitText()).toBe("___0");
    expect(caretIndex()).toBe(1);
  });

  it("moves the caret with the arrow keys", async () => {
    const user = UserEvent.setup();
    renderInner({ initialValue: "0930" });
    expect(caretIndex()).toBe(0);
    await user.keyboard("{ArrowRight}{ArrowRight}");
    expect(caretIndex()).toBe(2);
    await user.keyboard("{ArrowLeft}");
    expect(caretIndex()).toBe(1);
    // clamps at both ends
    await user.keyboard("{ArrowLeft}{ArrowLeft}{ArrowLeft}");
    expect(caretIndex()).toBe(0);
    await user.keyboard("{ArrowRight}{ArrowRight}{ArrowRight}{ArrowRight}");
    expect(caretIndex()).toBe(3);
  });

  it("confirms from the keyboard, resolving 12-hour entry against AM/PM", async () => {
    const user = UserEvent.setup();
    const onSave = vitest.fn();
    renderInner({ initialValue: "", onSave });
    await user.keyboard("0930");
    await user.keyboard("p");
    await user.keyboard("{Enter}");
    expect(onSave).toHaveBeenCalledTimes(1);
    expect(onSave.mock.calls[0][0]).toBe("startTime");
    expect(onSave.mock.calls[0][2]).toBe("2130");
  });

  it("does not confirm from the keyboard while digits are missing", async () => {
    const user = UserEvent.setup();
    const onSave = vitest.fn();
    renderInner({ initialValue: "09", onSave });
    await user.keyboard("{Enter}");
    expect(onSave).not.toHaveBeenCalled();
  });

  it("closes on Escape", async () => {
    const user = UserEvent.setup();
    const onClose = vitest.fn();
    renderInner({ onClose });
    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("offers Now on the sign-out field only", () => {
    renderInner({ field: "startTime" });
    expect(screen.queryByText("Now")).toBeNull();
    cleanup();
    renderInner({ field: "endTime" });
    expect(screen.getByText("Now")).toBeInTheDocument();
  });

  it("Now fills the current time and confirms to it", async () => {
    const user = UserEvent.setup();
    const onSave = vitest.fn();
    // 14:07 today, so the 12-hour entry (0207 PM) has to resolve back to 1407.
    const now = new Date();
    now.setHours(14, 7, 0, 0);
    vitest.setSystemTime(now);

    renderInner({ field: "endTime", initialValue: "", onSave });
    await user.click(screen.getByText("Now"));
    expect(digitText()).toBe("0207");
    expect(caretIndex()).toBe(0);

    await user.click(screen.getByText("Confirm"));
    expect(onSave).toHaveBeenCalledTimes(1);
    expect(onSave.mock.calls[0][0]).toBe("endTime");
    expect(onSave.mock.calls[0][2]).toBe("1407");
    // and it moved the date off the initial 2026-01-15 onto today
    expect(isSameDay(onSave.mock.calls[0][1], new Date())).toBe(true);
  });
});

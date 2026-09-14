import "@testing-library/jest-dom/vitest";
import { describe, it, expect } from "vitest";
import UserEvent from "@testing-library/user-event";
import { render, screen } from "@testing-library/react";
import { useState } from "react";
import TimeInputWithControls from "./TimeInputWithControls";

function Controlled({ initial = "2024-01-15T10:00" }: { initial?: string }) {
  const [value, setValue] = useState(initial);
  return (
    <TimeInputWithControls
      aria-label="time"
      value={value}
      onChange={setValue}
    />
  );
}

function ControlledPair() {
  const [start, setStart] = useState("2024-01-15T10:00");
  const [end, setEnd] = useState("2024-01-15T12:00");
  return (
    <>
      <TimeInputWithControls
        aria-label="start"
        value={start}
        onChange={setStart}
      />
      <TimeInputWithControls
        aria-label="end"
        value={end}
        onChange={setEnd}
        copyFrom={{ label: "Copy start time", value: start }}
      />
    </>
  );
}

describe("TimeInputWithControls", () => {
  it("nudges the value by an hour or a day in either direction", async () => {
    const user = UserEvent.setup();
    render(<Controlled />);
    const input = screen.getByLabelText("time") as HTMLInputElement;

    await user.click(screen.getByRole("button", { name: "+1h" }));
    expect(input.value).toBe("2024-01-15T11:00");

    await user.click(screen.getByRole("button", { name: "+1d" }));
    expect(input.value).toBe("2024-01-16T11:00");

    await user.click(screen.getByRole("button", { name: "-1h" }));
    expect(input.value).toBe("2024-01-16T10:00");

    await user.click(screen.getByRole("button", { name: "-1d" }));
    expect(input.value).toBe("2024-01-15T10:00");
  });

  it("copies another field's value in via the copyFrom button", async () => {
    const user = UserEvent.setup();
    render(<ControlledPair />);
    const endInput = screen.getByLabelText("end") as HTMLInputElement;

    expect(endInput.value).toBe("2024-01-15T12:00");
    await user.click(screen.getByRole("button", { name: "Copy start time" }));
    expect(endInput.value).toBe("2024-01-15T10:00");
  });

  it("disables the copy button when there's nothing to copy", () => {
    render(
      <TimeInputWithControls
        aria-label="end"
        value=""
        onChange={() => {}}
        copyFrom={{ label: "Copy start time", value: "" }}
      />,
    );
    expect(
      screen.getByRole("button", { name: "Copy start time" }),
    ).toBeDisabled();
  });
});

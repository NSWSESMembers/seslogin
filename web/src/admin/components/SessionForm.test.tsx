import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi } from "vitest";
import UserEvent from "@testing-library/user-event";
import { cleanup, render, screen, within } from "@testing-library/react";
import SessionForm from "./SessionForm";

// The config is only in React state until the form is submitted, so the way to
// see what an admin would save is to submit and read what the form posts.
async function pressWhosHereAndSave(
  initialConfig: Record<string, unknown>,
  label: string,
) {
  cleanup(); // a test may call this more than once
  const onSubmit = vi.fn();
  render(
    <SessionForm
      initialName="Kiosk"
      initialConfig={JSON.stringify(initialConfig)}
      initialHealthcheckUrl=""
      isMutationInFlight={false}
      onSubmit={onSubmit}
    />,
  );
  const user = UserEvent.setup();
  const group = screen.getByRole("group", { name: "Who's here" });
  await user.click(within(group).getByRole("button", { name: label }));
  await user.click(screen.getByRole("button", { name: "Save" }));
  const formData = onSubmit.mock.calls[0][0] as FormData;
  return JSON.parse(formData.get("config") as string);
}

describe("SessionForm Who's here", () => {
  it("offers a sign out choice that sets both always-visible keys", async () => {
    const config = await pressWhosHereAndSave({}, "Visible + Sign out");
    expect(config.signedInStatusInline).toBe(true);
    expect(config.signedInStatusSignOut).toBe(true);
    expect(config.signedInStatus).toBeUndefined();
  });

  it("clears the sign out key when switching to plain always visible", async () => {
    const config = await pressWhosHereAndSave(
      { signedInStatusInline: true, signedInStatusSignOut: true },
      "Always visible",
    );
    expect(config.signedInStatusInline).toBe(true);
    expect(config.signedInStatusSignOut).toBeUndefined();
  });

  it("clears the sign out key when switching to the button or off", async () => {
    const button = await pressWhosHereAndSave(
      { signedInStatusInline: true, signedInStatusSignOut: true },
      "Button",
    );
    expect(button.signedInStatus).toBe(true);
    expect(button.signedInStatusInline).toBeUndefined();
    expect(button.signedInStatusSignOut).toBeUndefined();

    const off = await pressWhosHereAndSave(
      { signedInStatusInline: true, signedInStatusSignOut: true },
      "Off",
    );
    expect(off.signedInStatus).toBeUndefined();
    expect(off.signedInStatusInline).toBeUndefined();
    expect(off.signedInStatusSignOut).toBeUndefined();
  });

  it("shows an existing sign out config as that choice, and leaves older configs alone", async () => {
    const { unmount } = render(
      <SessionForm
        initialName="Kiosk"
        initialConfig={JSON.stringify({
          signedInStatusInline: true,
          signedInStatusSignOut: true,
        })}
        initialHealthcheckUrl=""
        isMutationInFlight={false}
        onSubmit={vi.fn()}
      />,
    );
    const group = screen.getByRole("group", { name: "Who's here" });
    expect(
      within(group).getByRole("button", { name: "Visible + Sign out" }),
    ).toHaveAttribute("aria-pressed", "true");
    unmount();

    // A config saved before this option existed must still read as plain
    // always-visible, not as one that grew sign out buttons.
    render(
      <SessionForm
        initialName="Kiosk"
        initialConfig={JSON.stringify({ signedInStatusInline: true })}
        initialHealthcheckUrl=""
        isMutationInFlight={false}
        onSubmit={vi.fn()}
      />,
    );
    const oldGroup = screen.getByRole("group", { name: "Who's here" });
    expect(
      within(oldGroup).getByRole("button", { name: "Always visible" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      within(oldGroup).getByRole("button", {
        name: "Visible + Sign out",
      }),
    ).toHaveAttribute("aria-pressed", "false");
  });
});

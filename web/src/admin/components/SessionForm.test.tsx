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

describe("SessionForm Confirm visitors", () => {
  async function toggleAndSave(initialConfig: Record<string, unknown>) {
    cleanup();
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
    await user.click(
      screen.getByRole("checkbox", { name: /Confirm visitors/ }),
    );
    await user.click(screen.getByRole("button", { name: "Save" }));
    const formData = onSubmit.mock.calls[0][0] as FormData;
    return JSON.parse(formData.get("config") as string);
  }

  it("turns the option on", async () => {
    const config = await toggleAndSave({ guests: true });
    expect(config).toEqual({ guests: true, confirmVisitors: true });
  });

  it("drops the key when turned off", async () => {
    const config = await toggleAndSave({ confirmVisitors: true });
    expect(config).toEqual({});
  });
});

describe("SessionForm Category search", () => {
  it.each([
    { name: "off to start with, left alone", initial: undefined, click: false },
    { name: "on to start with, left alone", initial: true, click: false },
    { name: "turned on", initial: undefined, click: true },
    { name: "turned off", initial: true, click: true },
  ])(
    "saves category search $name and preserves other settings",
    async ({ initial, click }) => {
      const checked = !!initial !== click;
      const onSubmit = vi.fn();
      render(
        <SessionForm
          initialName="Kiosk"
          initialConfig={JSON.stringify({
            guests: true,
            categorySearch: initial,
          })}
          initialHealthcheckUrl=""
          isMutationInFlight={false}
          onSubmit={onSubmit}
        />,
      );
      const box = screen.getByRole("checkbox", { name: /Category search/ });
      expect(box).toHaveProperty("checked", !!initial);
      const user = UserEvent.setup();
      if (click) await user.click(box);
      await user.click(screen.getByRole("button", { name: "Save" }));
      const data = onSubmit.mock.calls[0][0] as FormData;
      expect(JSON.parse(data.get("config") as string)).toEqual(
        checked ? { guests: true, categorySearch: true } : { guests: true },
      );
    },
  );
});

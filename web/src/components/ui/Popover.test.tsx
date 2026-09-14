import { beforeEach, describe, expect, test, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import UserEvent from "@testing-library/user-event";
import { useRef, useState } from "react";
import { Popover } from "./Popover";

/**
 * Popover shares its measure/dismiss engine with Combobox's listbox
 * (`useAnchoredPopup`), so these pin the tooltip half of that contract: centred
 * under its anchor, and dismissed by an outside click, Escape, or the viewport
 * moving.
 *
 * Behaviour only, never geometry — `getBoundingClientRect` returns zeros in
 * jsdom, so where the panel lands can only be checked in a real browser.
 */
function Harness({
  onDismiss,
  role,
  className,
}: {
  onDismiss?: () => void;
  role?: string;
  className?: string;
}) {
  const anchorRef = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  return (
    <div>
      <button ref={anchorRef} onClick={() => setOpen((v) => !v)}>
        Trigger
      </button>
      <span>outside</span>
      {open && (
        <Popover
          anchorRef={anchorRef}
          role={role}
          className={className}
          onDismiss={() => {
            onDismiss?.();
            setOpen(false);
          }}
        >
          <p>Panel body</p>
        </Popover>
      )}
    </div>
  );
}

function panel() {
  return screen.queryByRole("tooltip");
}

async function openPanel(user: ReturnType<typeof UserEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Trigger" }));
  expect(panel()).toBeInTheDocument();
}

beforeEach(() => {
  window.innerWidth = 1024;
});

describe("Popover", () => {
  test("renders nothing until opened, then portals its content to the body", async () => {
    const user = UserEvent.setup();
    const { container } = render(<Harness />);
    expect(panel()).not.toBeInTheDocument();

    await openPanel(user);
    expect(screen.getByText("Panel body")).toBeInTheDocument();
    // Portalled: outside the component's own container, under <body>.
    expect(container).not.toContainElement(panel());
    expect(document.body).toContainElement(panel());
  });

  test("applies the given role and className", async () => {
    const user = UserEvent.setup();
    render(<Harness role="dialog" className="max-w-xs" />);
    await user.click(screen.getByRole("button", { name: "Trigger" }));

    const el = screen.getByRole("dialog");
    expect(el).toHaveClass("max-w-xs");
    // The shared chrome still comes from the component, not the caller.
    expect(el).toHaveClass("fixed", "-translate-x-1/2");
  });

  test("dismisses on an outside click", async () => {
    const onDismiss = vi.fn();
    const user = UserEvent.setup();
    render(<Harness onDismiss={onDismiss} />);
    await openPanel(user);

    await user.click(screen.getByText("outside"));
    expect(onDismiss).toHaveBeenCalled();
    expect(panel()).not.toBeInTheDocument();
  });

  test("stays open when the panel itself is clicked", async () => {
    const onDismiss = vi.fn();
    const user = UserEvent.setup();
    render(<Harness onDismiss={onDismiss} />);
    await openPanel(user);

    await user.click(screen.getByText("Panel body"));
    expect(onDismiss).not.toHaveBeenCalled();
    expect(panel()).toBeInTheDocument();
  });

  test("dismisses on Escape", async () => {
    const onDismiss = vi.fn();
    const user = UserEvent.setup();
    render(<Harness onDismiss={onDismiss} />);
    await openPanel(user);

    await user.keyboard("{Escape}");
    expect(onDismiss).toHaveBeenCalled();
    expect(panel()).not.toBeInTheDocument();
  });

  test("dismisses when the viewport scrolls or resizes", async () => {
    const user = UserEvent.setup();
    render(<Harness />);

    await openPanel(user);
    act(() => {
      window.dispatchEvent(new Event("scroll"));
    });
    expect(panel()).not.toBeInTheDocument();

    await openPanel(user);
    act(() => {
      window.dispatchEvent(new Event("resize"));
    });
    expect(panel()).not.toBeInTheDocument();
  });

  test("ignores a scroll that originates inside the panel", async () => {
    const onDismiss = vi.fn();
    const user = UserEvent.setup();
    render(<Harness onDismiss={onDismiss} />);
    await openPanel(user);

    // A scrollable panel scrolling itself is not the viewport moving. The
    // listener is capture-phase on window, so without the check it would see
    // this and close.
    act(() => {
      screen
        .getByText("Panel body")
        .dispatchEvent(new Event("scroll", { bubbles: false }));
    });
    expect(onDismiss).not.toHaveBeenCalled();
    expect(panel()).toBeInTheDocument();
  });

  test("stops listening once dismissed", async () => {
    const onDismiss = vi.fn();
    const user = UserEvent.setup();
    render(<Harness onDismiss={onDismiss} />);
    await openPanel(user);

    await user.keyboard("{Escape}");
    expect(onDismiss).toHaveBeenCalledTimes(1);

    // Torn down: further Escapes and scrolls must not reach the closed panel.
    await user.keyboard("{Escape}");
    act(() => {
      window.dispatchEvent(new Event("scroll"));
    });
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });
});

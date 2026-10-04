import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi } from "vitest";
import { act } from "react";
import UserEvent from "@testing-library/user-event";
import { render, waitFor } from "@testing-library/react";
import { RelayEnvironmentProvider } from "react-relay";
import { MockPayloadGenerator, createMockEnvironment } from "relay-test-utils";
import Footer from "./Footer";
import { NotifyContext, type NotifyContextValue } from "./useNotify";

// relay-test-utils reaches for jest.fn() internally; shim it for vitest.
(globalThis as unknown as { jest: { fn: typeof vi.fn } }).jest = { fn: vi.fn };

function renderFooter() {
  const environment = createMockEnvironment();
  const notify: NotifyContextValue = {
    notify: vi.fn(),
    notifySuccess: vi.fn(),
    notifyError: vi.fn(),
    dismiss: vi.fn(),
  };
  const screen = render(
    <RelayEnvironmentProvider environment={environment}>
      <NotifyContext.Provider value={notify}>
        <Footer />
      </NotifyContext.Provider>
    </RelayEnvironmentProvider>,
  );
  return { environment, notify, screen };
}

async function openHelp(
  screen: ReturnType<typeof render>,
  environment: ReturnType<typeof createMockEnvironment>,
  feedbackAvailable: boolean,
) {
  const link = screen.getByRole("link", { name: "Help/Feedback?" });
  expect(link).toHaveAttribute("href", "mailto:support@seslogin.com");
  await UserEvent.setup().click(link);
  act(() => {
    environment.mock.resolveMostRecentOperation((op) =>
      MockPayloadGenerator.generate(op, {
        Query: () => ({ feedbackAvailable }),
        User: () => ({ email: "admin@example.com" }),
      }),
    );
  });
}

describe("Footer support link", () => {
  it("opens the help form and sends the message", async () => {
    const { environment, notify, screen } = renderFooter();
    await openHelp(screen, environment, true);

    expect(
      await screen.findByText(/Send the seslogin team a message/),
    ).toBeInTheDocument();
    expect(screen.getByText("ask for help using seslogin")).toBeInTheDocument();
    expect(screen.getByText("admin@example.com")).toBeInTheDocument();
    expect(
      screen.queryByText(/Email the seslogin team/),
    ).not.toBeInTheDocument();

    const user = UserEvent.setup();
    await user.type(screen.getByLabelText("Subject"), "Report is empty");
    await user.type(screen.getByLabelText("Message"), "Line one{enter}Two");
    await user.click(screen.getByRole("button", { name: "Send" }));

    const op = environment.mock.getMostRecentOperation();
    expect(op.request.node.params.name).toBe("HelpFormMutation");
    expect(op.request.variables).toEqual({
      subject: "Report is empty",
      message: "Line one\nTwo",
    });
    act(() => {
      environment.mock.resolve(
        op,
        MockPayloadGenerator.generate(op, {
          SubmittedFeedback: () => ({ number: 7, reference: "[#help-7]" }),
        }),
      );
    });

    await waitFor(() =>
      expect(notify.notifySuccess).toHaveBeenCalledWith(
        expect.stringContaining("[#help-7]"),
      ),
    );
    expect(screen.queryByLabelText("Subject")).not.toBeInTheDocument();
  });

  it("offers only email when the form isn't configured", async () => {
    const { environment, screen } = renderFooter();
    await openHelp(screen, environment, false);

    expect(
      await screen.findByText(/Email the seslogin team at/),
    ).toBeInTheDocument();
    expect(screen.getByText("ask for help using seslogin")).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "Write an email" }),
    ).toHaveAttribute("href", "mailto:support@seslogin.com");
    expect(screen.queryByLabelText("Subject")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Send" })).toBeNull();
    expect(screen.queryByText(/We.ll reply by email/)).toBeNull();

    await UserEvent.setup().click(
      screen.getByRole("button", { name: "Close" }),
    );
    expect(screen.queryByText(/Email the seslogin team at/)).toBeNull();
  });
});

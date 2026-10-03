import "@testing-library/jest-dom/vitest";
import { describe, it, expect } from "vitest";
import UserEvent from "@testing-library/user-event";
import { render } from "@testing-library/react";
import HelpLink from "./HelpLink";
import { EmailHelp } from "./EmailHelp";

describe("HelpLink with EmailHelp (home page footer)", () => {
  it("opens an email-only dialog and closes it", async () => {
    const screen = render(
      <HelpLink>{(close) => <EmailHelp onClose={close} />}</HelpLink>,
    );
    const user = UserEvent.setup();

    const link = screen.getByRole("link", { name: "Help/Feedback?" });
    expect(link).toHaveAttribute("href", "mailto:support@seslogin.com");
    await user.click(link);

    expect(screen.getByText(/Email the seslogin team at/)).toBeInTheDocument();
    expect(screen.getByText("ask for help using seslogin")).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "Write an email" }),
    ).toHaveAttribute("href", "mailto:support@seslogin.com");
    expect(screen.queryByRole("textbox")).toBeNull();

    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.queryByText(/Email the seslogin team at/)).toBeNull();
  });
});

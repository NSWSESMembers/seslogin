import "@testing-library/jest-dom/vitest";
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import UserEvent from "@testing-library/user-event";
import { render, screen, waitFor } from "@testing-library/react";
import { setupServer } from "msw/node";
import { graphql, HttpResponse } from "msw";

import { getGraphQLEndpoint } from "../lib/api";
import TallyAddPasskey from "./TallyAddPasskey";

const passkey = vi.hoisted(() => ({ create: vi.fn() }));
vi.mock("../lib/passkey", () => ({
  browserSupportsWebAuthn: () => true,
  createPasskey: passkey.create,
}));

const endpoint = graphql.link(getGraphQLEndpoint());
const finishCalls: Record<string, unknown>[] = [];
const sendCalls: Record<string, unknown>[] = [];

const server = setupServer(
  endpoint.mutation("SendTallyPasskeySetupCode", ({ variables }) => {
    sendCalls.push(variables);
    return variables.ticket === "good"
      ? HttpResponse.json({ data: { sendTallyPasskeySetupCode: true } })
      : HttpResponse.json({
          data: null,
          errors: [
            {
              message: "This link has expired. Go back to Tally and try again.",
              extensions: { code: "BAD_REQUEST" },
            },
          ],
        });
  }),
  endpoint.mutation("VerifyTallyPasskeySetupCode", ({ variables }) =>
    variables.code === "123456"
      ? HttpResponse.json({ data: { verifyTallyPasskeySetupCode: true } })
      : HttpResponse.json({
          data: null,
          errors: [
            {
              message:
                "That code is wrong or has expired. Check it, or send a new one.",
              extensions: { code: "BAD_REQUEST" },
            },
          ],
        }),
  ),
  endpoint.mutation("BeginTallyPasskeySetup", ({ variables }) =>
    variables.ticket === "good"
      ? HttpResponse.json({
          data: {
            beginTallyPasskeySetup: {
              challengeId: "ch1",
              optionsJson: "{}",
            },
          },
        })
      : HttpResponse.json({
          data: null,
          errors: [
            {
              message: "This link has expired. Go back to Tally and try again.",
              extensions: { code: "BAD_REQUEST" },
            },
          ],
        }),
  ),
  endpoint.mutation("FinishTallyPasskeySetup", ({ variables }) => {
    finishCalls.push(variables);
    return HttpResponse.json({
      data: {
        finishTallyPasskeySetup: "tally://oauth/callback?passkey=saved",
      },
    });
  }),
);

beforeAll(() => server.listen());
afterEach(() => {
  server.resetHandlers();
  finishCalls.length = 0;
  sendCalls.length = 0;
  vi.unstubAllGlobals();
});
afterAll(() => server.close());

const assign = vi.fn();
beforeEach(() => {
  assign.mockReset();
  passkey.create.mockReset();
});

function renderWithTicket(hash: string) {
  vi.stubGlobal("location", {
    ...window.location,
    pathname: "/tally/passkey",
    search: "",
    hash,
    assign,
  });
  return render(<TallyAddPasskey />);
}

describe("TallyAddPasskey", () => {
  it("refuses a link with no ticket", () => {
    renderWithTicket("");
    expect(screen.getByText("Can't add a passkey here")).toBeInTheDocument();
  });

  /** Confirm the email with the right (or a given) code. */
  async function confirmEmail(
    user: ReturnType<typeof UserEvent.setup>,
    code = "123456",
  ) {
    await user.type(
      screen.getByPlaceholderText("Email address"),
      "alice@example.com",
    );
    await user.click(screen.getByRole("button", { name: "Send code" }));
    await user.type(await screen.findByLabelText("6-digit code"), code);
    await user.click(screen.getByRole("button", { name: "Confirm" }));
  }

  it("confirms the email with a code, saves a passkey and returns to Tally", async () => {
    const user = UserEvent.setup();
    passkey.create.mockResolvedValue({
      status: "ok",
      credentialJson: '{"id":"x"}',
    });
    renderWithTicket("#good");

    await confirmEmail(user);
    await user.click(
      await screen.findByRole("button", { name: "Add a passkey" }),
    );

    await waitFor(() =>
      expect(assign).toHaveBeenCalledWith(
        "tally://oauth/callback?passkey=saved",
      ),
    );
    expect(sendCalls[0]).toEqual({
      ticket: "good",
      email: "alice@example.com",
    });
    expect(finishCalls[0]).toEqual({
      ticket: "good",
      challengeId: "ch1",
      credentialJson: '{"id":"x"}',
    });
  });

  it("offers no passkey until the code is right", async () => {
    const user = UserEvent.setup();
    renderWithTicket("#good");

    await confirmEmail(user, "000000");

    expect(
      await screen.findByText(
        "That code is wrong or has expired. Check it, or send a new one.",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Add a passkey" })).toBeNull();
  });

  it("says so when no passkey was made, and lets them try again", async () => {
    const user = UserEvent.setup();
    passkey.create.mockResolvedValue({ status: "cancelled" });
    renderWithTicket("#good");

    await confirmEmail(user);
    await user.click(
      await screen.findByRole("button", { name: "Add a passkey" }),
    );

    expect(
      await screen.findByText("No passkey was saved. Please try again."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add a passkey" })).toBeEnabled();
    expect(assign).not.toHaveBeenCalled();
  });

  it("shows the server's words for an expired ticket", async () => {
    const user = UserEvent.setup();
    renderWithTicket("#stale");

    await user.type(
      screen.getByPlaceholderText("Email address"),
      "alice@example.com",
    );
    await user.click(screen.getByRole("button", { name: "Send code" }));

    expect(
      await screen.findByText(
        "This link has expired. Go back to Tally and try again.",
      ),
    ).toBeInTheDocument();
    expect(passkey.create).not.toHaveBeenCalled();
  });
});

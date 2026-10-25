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
import { MemoryRouter, Route, Routes } from "react-router";
import { setupServer } from "msw/node";
import { graphql, HttpResponse } from "msw";

import { getGraphQLEndpoint } from "../lib/api";
import TallySignIn from "./TallySignIn";

const LINK =
  "/tally/sign-in?response_type=code&client_id=tally" +
  "&redirect_uri=tally%3A%2F%2Foauth%2Fcallback" +
  "&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM" +
  "&code_challenge_method=S256&state=xyz";

const endpoint = graphql.link(getGraphQLEndpoint());

const completeCalls: Record<string, unknown>[] = [];

const server = setupServer(
  endpoint.query("TallySignInRequest", ({ variables }) =>
    variables.clientId === "tally"
      ? HttpResponse.json({
          data: {
            tallySignInRequest: {
              clientName: "Tally",
              isDevelopmentBuild: false,
            },
          },
        })
      : HttpResponse.json({
          data: null,
          errors: [{ message: "Unknown client" }],
        }),
  ),
  endpoint.mutation("RequestTallySignInCode", () =>
    HttpResponse.json({ data: { requestTallySignInCode: true } }),
  ),
  endpoint.mutation("CompleteTallySignIn", ({ variables }) => {
    completeCalls.push(variables);
    return variables.code === "123456"
      ? HttpResponse.json({
          data: {
            completeTallySignIn: "tally://oauth/callback?code=abc&state=xyz",
          },
        })
      : HttpResponse.json({
          data: null,
          errors: [{ message: "That code is wrong or has expired" }],
        });
  }),
);

beforeAll(() => server.listen());
afterEach(() => {
  server.resetHandlers();
  completeCalls.length = 0;
});
afterAll(() => server.close());

const assign = vi.fn();
beforeEach(() => {
  assign.mockReset();
  vi.stubGlobal("location", { ...window.location, assign });
});
afterEach(() => vi.unstubAllGlobals());

function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path="/tally/sign-in" element={<TallySignIn />} />
      </Routes>
    </MemoryRouter>,
  );
}

describe("TallySignIn", () => {
  it("refuses a link that isn't an authorization request", async () => {
    renderAt("/tally/sign-in?client_id=tally");
    expect(await screen.findByText("Can't sign in here")).toBeInTheDocument();
  });

  it("refuses a link the server doesn't recognise", async () => {
    renderAt(LINK.replace("client_id=tally", "client_id=other"));
    expect(await screen.findByText("Can't sign in here")).toBeInTheDocument();
  });

  it("sends a code, then returns to the app with the authorization code", async () => {
    const user = UserEvent.setup();
    renderAt(LINK);

    expect(await screen.findByText("Sign in to Tally")).toBeInTheDocument();
    await user.type(
      screen.getByPlaceholderText("Email address"),
      "alice@example.com",
    );
    await user.click(screen.getByRole("button", { name: "Send code" }));

    await user.type(await screen.findByLabelText("6-digit code"), "123456");
    await user.click(screen.getByRole("button", { name: "Sign in" }));

    await waitFor(() =>
      expect(assign).toHaveBeenCalledWith(
        "tally://oauth/callback?code=abc&state=xyz",
      ),
    );
    expect(completeCalls[0]).toEqual({
      email: "alice@example.com",
      code: "123456",
      request: {
        clientId: "tally",
        redirectUri: "tally://oauth/callback",
        codeChallenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
        codeChallengeMethod: "S256",
        scope: null,
        resource: null,
        state: "xyz",
      },
    });
  });

  it("says so when the code is wrong, and lets them try again", async () => {
    const user = UserEvent.setup();
    renderAt(LINK);

    await user.type(
      await screen.findByPlaceholderText("Email address"),
      "alice@example.com",
    );
    await user.click(screen.getByRole("button", { name: "Send code" }));
    await user.type(await screen.findByLabelText("6-digit code"), "000000");
    await user.click(screen.getByRole("button", { name: "Sign in" }));

    expect(
      await screen.findByText(/That code is wrong or has expired/),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("6-digit code")).toHaveValue("");
    expect(assign).not.toHaveBeenCalled();
  });
});

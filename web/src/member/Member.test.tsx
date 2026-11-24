import "@testing-library/jest-dom/vitest";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import UserEvent from "@testing-library/user-event";
import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { setupServer } from "msw/node";
import { graphql, HttpResponse } from "msw";

import { getGraphQLEndpoint } from "../lib/api";
import MemberSignIn from "./MemberSignIn";
import MemberSettings from "./MemberSettings";

const endpoint = graphql.link(getGraphQLEndpoint());

const GOOD = "slms_good";
let remindersOn = true;
const updates: Record<string, unknown>[] = [];
const signOuts: Record<string, unknown>[] = [];

const UNAUTHENTICATED = {
  data: null,
  errors: [
    {
      message: "This link has expired or is no longer valid",
      extensions: { code: "UNAUTHENTICATED" },
    },
  ],
};

const settings = () => ({
  firstName: "Alice",
  lastName: "Anderson",
  email: "alice@example.com",
  signedInReminders: remindersOn,
});

const server = setupServer(
  endpoint.query("MemberSettings", ({ variables }) =>
    variables.token === GOOD
      ? HttpResponse.json({ data: { memberSettings: settings() } })
      : HttpResponse.json(UNAUTHENTICATED),
  ),
  endpoint.mutation("UpdateMemberSettings", ({ variables }) => {
    updates.push(variables);
    if (variables.token !== GOOD) return HttpResponse.json(UNAUTHENTICATED);
    remindersOn = variables.signedInReminders as boolean;
    return HttpResponse.json({ data: { updateMemberSettings: settings() } });
  }),
  endpoint.mutation("SignOutOfMemberSettings", ({ variables }) => {
    signOuts.push(variables);
    return HttpResponse.json({ data: { signOutOfMemberSettings: true } });
  }),
  endpoint.mutation("RequestMemberSettingsCode", () =>
    HttpResponse.json({ data: { requestMemberSettingsCode: true } }),
  ),
  endpoint.mutation("SignInToMemberSettings", ({ variables }) =>
    variables.code === "123456"
      ? HttpResponse.json({ data: { signInToMemberSettings: GOOD } })
      : HttpResponse.json({
          data: null,
          errors: [
            {
              message: "That code is wrong or has expired",
              extensions: { code: "BAD_REQUEST" },
            },
          ],
        }),
  ),
);

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => {
  server.resetHandlers();
  remindersOn = true;
  updates.length = 0;
  signOuts.length = 0;
});
afterAll(() => server.close());

/** Shows where the app is, so a redirect can be checked. */
function Where() {
  const { pathname, hash } = useLocation();
  return <div data-testid="where">{pathname + hash}</div>;
}

function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path="/member/sign-in" element={<MemberSignIn />} />
        <Route path="/member/settings" element={<MemberSettings />} />
      </Routes>
      <Where />
    </MemoryRouter>,
  );
}

describe("MemberSettings", () => {
  it("shows the setting and saves a change", async () => {
    const user = UserEvent.setup();
    renderAt(`/member/settings#token=${GOOD}&email=alice%40example.com`);

    const box = await screen.findByRole("checkbox", {
      name: /remind me when i forget to sign out/i,
    });
    expect(screen.getByText(/Alice Anderson/)).toBeInTheDocument();
    expect(box).toBeChecked();

    await user.click(box);
    await screen.findByText("Saved.");
    expect(box).not.toBeChecked();
    expect(updates).toEqual([{ token: GOOD, signedInReminders: false }]);
  });

  it("sends an expired link to sign in with the email filled in", async () => {
    renderAt("/member/settings#token=slms_old&email=alice%40example.com");

    expect(
      await screen.findByText(/that link has expired/i),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Email address")).toHaveValue(
      "alice@example.com",
    );
  });

  it("sends a link with no token to sign in", async () => {
    renderAt("/member/settings");
    expect(await screen.findByLabelText("Email address")).toHaveValue("");
    expect(screen.queryByText(/that link has expired/i)).toBeNull();
  });

  it("signs out", async () => {
    const user = UserEvent.setup();
    renderAt(`/member/settings#token=${GOOD}&email=alice%40example.com`);
    await user.click(await screen.findByRole("button", { name: "Sign out" }));
    await waitFor(() =>
      expect(screen.getByTestId("where")).toHaveTextContent(
        "/member/sign-in#email=alice%40example.com",
      ),
    );
    expect(signOuts).toEqual([{ token: GOOD }]);
  });
});

describe("MemberSignIn", () => {
  it("signs in with an emailed code and opens the settings", async () => {
    const user = UserEvent.setup();
    renderAt("/member/sign-in");

    await user.type(
      screen.getByLabelText("Email address"),
      "Alice@Example.com",
    );
    await user.click(screen.getByRole("button", { name: "Send code" }));

    const code = await screen.findByLabelText("6-digit code");
    await user.type(code, "000000");
    await user.click(screen.getByRole("button", { name: "Sign in" }));
    expect(
      await screen.findByText(/that code is wrong or has expired/i),
    ).toBeInTheDocument();

    await user.type(screen.getByLabelText("6-digit code"), "123456");
    await user.click(screen.getByRole("button", { name: "Sign in" }));

    expect(
      await screen.findByRole("checkbox", {
        name: /remind me when i forget to sign out/i,
      }),
    ).toBeChecked();
    expect(screen.getByTestId("where")).toHaveTextContent(
      `/member/settings#token=${GOOD}&email=alice%40example.com`,
    );
  });
});

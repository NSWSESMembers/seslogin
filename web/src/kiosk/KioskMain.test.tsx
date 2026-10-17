import "@testing-library/jest-dom/vitest";
import { vi, describe, it, expect, beforeEach } from "vitest";
import UserEvent from "@testing-library/user-event";
import type { UserEvent as UserEventInstance } from "@testing-library/user-event";
import { act, render, screen, waitFor } from "@testing-library/react";
import { getGraphQLEndpoint } from "../lib/api";
import { beforeAll, afterEach, afterAll } from "vitest";
import { setupServer } from "msw/node";
import { graphql, HttpResponse } from "msw";
import KioskMain from "./KioskMain";

const FOUND_USER = "40050107";
const FOUND_USER_RESPONSE = {
  data: {
    scanRegister2: {
      id: FOUND_USER,
      state: "SIGNED_IN",
      period: {
        id: "period-123",
        version: 1,
        startTime: new Date().getTime() - 1000 * 60 * 60,
        endTime: new Date().getTime(),
        person: {
          id: `person-${FOUND_USER}`,
          firstName: "Random",
          lastName: "Guy",
        },
      },
      quickPick: null,
    },
  },
};
const SIGNOUT_USER = "40050108";
function signOutUserResponse(quickPick: unknown = null) {
  return {
    data: {
      scanRegister2: {
        id: SIGNOUT_USER,
        state: "SIGN_OUT_PENDING",
        period: {
          id: "period-456",
          version: 1,
          startTime: new Date().getTime() - 1000 * 60 * 60,
          endTime: null,
          person: {
            id: `person-${SIGNOUT_USER}`,
            firstName: "Jamie",
            lastName: "Smith",
          },
        },
        quickPick,
      },
    },
  };
}
// a real leaf category id from the category tree in
// web/src/lib/categories.ts (Training > AIIMS)
const QUICK_PICK_CATEGORY_ID = "RX2bfpU6ppvV";
const SETTINGS = {
  scanAuthToken: "mock-token",
  scanAuthTokenIssuedAt: new Date().getTime(),
};

const relayUrl = getGraphQLEndpoint();
const relayEndpoint = graphql.link(relayUrl);

function sessionConfigHandler(config: Record<string, unknown>) {
  return relayEndpoint.query("KioskTokenSessionFetcherQuery", () => {
    return HttpResponse.json({
      data: {
        refresh_token: "not-a-refreshed-token",
        session: {
          id: "mockId",
          name: "mockName",
          config,
          version: 1,
          location: {
            id: "mockLocationId",
            name: "mockLocation",
          },
        },
      },
    });
  });
}

// LivePeriodsProvider's first move, on any kiosk with a status/guest view
// enabled, is to ask for a realtime token. Returning null keeps every test
// below on the polling fallback rather than needing a fake Ably client — see
// LivePeriodsProvider.test.tsx for the realtime path itself.
function kioskRealtimeTokenHandler() {
  return relayEndpoint.query("KioskRealtimeTokenQuery", () =>
    HttpResponse.json({ data: { kioskRealtimeToken: null } }),
  );
}

// The one snapshot query behind Status, ScanStatusDialog and
// ScanSignedInPanel now that all three read from LivePeriodsProvider.
function snapshotHandler(
  nodes: Array<{
    id: string;
    version: number;
    startTime: number;
    guestName: string | null;
    person: { id: string; firstName: string; lastName: string } | null;
  }>,
) {
  return relayEndpoint.query("LivePeriodsSnapshotQuery", () =>
    HttpResponse.json({
      data: {
        session: {
          location: { periods: { edges: nodes.map((node) => ({ node })) } },
        },
      },
    }),
  );
}

const EMPTY_QUICK_PICK = { locationCategories: [], personCategories: [] };

const POPULATED_QUICK_PICK = {
  locationCategories: [
    {
      category: { id: QUICK_PICK_CATEGORY_ID },
      recentPeople: [
        { id: "person-jane", firstName: "Jane" },
        { id: "person-tom", firstName: "Tom" },
      ],
    },
  ],
  personCategories: [{ category: { id: QUICK_PICK_CATEGORY_ID } }],
};

// The quick pick now rides along with the register mutation, so a test picks its
// shape by swapping this handler rather than by stubbing a second query.
function register2Handler(quickPick: unknown = null) {
  return relayEndpoint.mutation(
    "ScanControllerRegister2Mutation",
    ({ variables }) => {
      const { memberNumber } = variables;
      if (memberNumber === FOUND_USER) {
        return HttpResponse.json(FOUND_USER_RESPONSE);
      }
      if (memberNumber === SIGNOUT_USER) {
        return HttpResponse.json(signOutUserResponse(quickPick));
      }
      return HttpResponse.json({
        data: {
          scanRegister2: {
            id: memberNumber,
            state: "NOT_FOUND",
            period: null,
            quickPick: null,
          },
        },
      });
    },
  );
}

const graphqlHandlers = [
  sessionConfigHandler({}),
  register2Handler(),
  kioskRealtimeTokenHandler(),
];

const server = setupServer(...graphqlHandlers);
const getItemSpy = vi.spyOn(localStorage, "getItem");
const audioPlaySpy = vi.spyOn(HTMLAudioElement.prototype, "play");

beforeAll(() => {
  server.listen();
});
beforeEach(() => {
  vi.spyOn(console, "log").mockImplementation(() => {});
  getItemSpy.mockReturnValue(JSON.stringify(SETTINGS));
});
afterEach(() => {
  server.resetHandlers();
  getItemSpy.mockClear();
  audioPlaySpy.mockClear();
});
afterAll(() => {
  server.close();
});

async function setupTest() {
  render(<KioskMain />);

  expect(getItemSpy).toHaveBeenCalledOnce();

  await waitFor(() =>
    expect(
      screen.getByText("Please enter or scan your SES ID"),
    ).toBeInTheDocument(),
  );

  return UserEvent.setup();
}

describe("KioskMain", () => {
  it("renders the main screen given a valid session", async () => {
    await setupTest();
  });

  it("rejects an incorrectly entered member ID", async () => {
    const user = await setupTest();

    await user.type(screen.getByRole("textbox"), "invalid-id{enter}");
    await waitFor(() =>
      expect(
        screen.getByText("Member ID must be at least 8 digits long"),
      ).toBeInTheDocument(),
    );
    expect(audioPlaySpy).toHaveBeenCalledOnce();
  });

  it("accepts a correctly entered member ID", async () => {
    const user = await setupTest();
    const textbox = screen.getByRole("textbox");
    await user.type(textbox, FOUND_USER);
    expect(textbox).toHaveValue(FOUND_USER);
    await user.type(textbox, "{enter}");
    await waitFor(() =>
      expect(
        screen.getByText(
          FOUND_USER_RESPONSE.data.scanRegister2.period.person.firstName +
            " " +
            FOUND_USER_RESPONSE.data.scanRegister2.period.person.lastName,
        ),
      ).toBeInTheDocument(),
    );
    expect(audioPlaySpy).toHaveBeenCalledOnce();
    expect(textbox).toHaveValue("");
  });

  it("returns an error for a member ID that does not exist", async () => {
    const user = await setupTest();
    await user.type(screen.getByRole("textbox"), "40050100{enter}");
    await waitFor(() =>
      expect(
        screen.getByText("Unknown member ID: 40050100"),
      ).toBeInTheDocument(),
    );
    expect(audioPlaySpy).toHaveBeenCalledOnce();
  });

  it("reports a recorded-but-undisplayed scan when the mutation response has a field error", async () => {
    // The write succeeded (data is non-null) but a nested field — Period.person,
    // as a dangling reference would produce — failed to resolve. This must not
    // be silently treated as a successful scan with a missing name.
    server.use(
      relayEndpoint.mutation("ScanControllerRegister2Mutation", () => {
        return HttpResponse.json({
          data: {
            scanRegister2: {
              id: FOUND_USER,
              state: "SIGNED_IN",
              period: {
                id: "period-123",
                startTime: new Date().getTime() - 1000 * 60 * 60,
                endTime: new Date().getTime(),
                person: null,
              },
              quickPick: null,
            },
          },
          errors: [
            {
              message: "Person with ID abc123 missing",
              path: ["scanRegister2", "period", "person"],
            },
          ],
        });
      }),
    );
    const user = await setupTest();
    await user.type(screen.getByRole("textbox"), FOUND_USER + "{enter}");
    await waitFor(() =>
      expect(
        screen.getByText(
          `recorded a scan for member ID ${FOUND_USER}, but couldn't display the result — check the activity list in admin`,
        ),
      ).toBeInTheDocument(),
    );
    expect(audioPlaySpy).toHaveBeenCalledOnce();
  });
});

describe("KioskMain theme", () => {
  // `data-theme` is stamped from a passive effect, which React may run on a
  // later task than the commit that renders the scan screen. `setupTest` only
  // waits for that text, so a pinned theme has to be waited for in turn.
  async function expectTheme(theme: string) {
    await waitFor(() =>
      expect(document.documentElement).toHaveAttribute("data-theme", theme),
    );
  }

  afterEach(() => {
    document.documentElement.removeAttribute("data-theme");
  });

  it("leaves the theme unpinned when no theme is set", async () => {
    await setupTest();
    expect(document.documentElement).not.toHaveAttribute("data-theme");
  });

  it("pins light when the theme is 'light'", async () => {
    server.use(sessionConfigHandler({ theme: "light" }));
    await setupTest();
    await expectTheme("light");
  });

  it("pins dark when the theme is 'dark'", async () => {
    server.use(sessionConfigHandler({ theme: "dark" }));
    await setupTest();
    await expectTheme("dark");
  });

  it("leaves the theme unpinned when the theme is 'auto'", async () => {
    server.use(sessionConfigHandler({ theme: "auto" }));
    await setupTest();
    expect(document.documentElement).not.toHaveAttribute("data-theme");
  });

  it("pins light for an unrecognised theme value", async () => {
    server.use(sessionConfigHandler({ theme: "solarized" }));
    await setupTest();
    await expectTheme("light");
  });
});

describe("KioskMain quick pick categories", () => {
  async function setupQuickPickTest(quickPick: unknown) {
    server.use(
      sessionConfigHandler({ quickPickCategories: true }),
      register2Handler(quickPick),
    );
    const user = await setupTest();
    await user.type(screen.getByRole("textbox"), SIGNOUT_USER + "{enter}");
    return user;
  }

  it("shows location and personal recent categories, and selecting one proceeds to confirm", async () => {
    const user = await setupQuickPickTest(POPULATED_QUICK_PICK);

    await waitFor(() =>
      expect(screen.getByText("Quick pick")).toBeInTheDocument(),
    );
    expect(screen.getByText("This location")).toBeInTheDocument();
    expect(screen.getByText("You")).toBeInTheDocument();
    expect(screen.getByText("Jane, Tom")).toBeInTheDocument();

    await user.click(screen.getAllByRole("button", { name: /AIIMS/ })[0]);

    await waitFor(() =>
      expect(screen.getByText("Confirm")).toBeInTheDocument(),
    );
  });

  it("falls through to the full category tree via 'More categories'", async () => {
    const user = await setupQuickPickTest(POPULATED_QUICK_PICK);

    await waitFor(() =>
      expect(screen.getByText("Quick pick")).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: "More categories" }));

    await waitFor(() =>
      expect(screen.getByText("Categories")).toBeInTheDocument(),
    );
  });

  it("skips straight to the full category tree when there are no recent categories", async () => {
    await setupQuickPickTest(EMPTY_QUICK_PICK);

    await waitFor(() =>
      expect(screen.getByText("Categories")).toBeInTheDocument(),
    );
    expect(screen.queryByText("Quick pick")).not.toBeInTheDocument();
  });

  it("skips straight to the full category tree when the server sends no quick pick", async () => {
    await setupQuickPickTest(null);

    await waitFor(() =>
      expect(screen.getByText("Categories")).toBeInTheDocument(),
    );
    expect(screen.queryByText("Quick pick")).not.toBeInTheDocument();
  });

  it("only asks for the quick pick when the kiosk has the screen enabled", async () => {
    const seen: unknown[] = [];
    server.use(
      sessionConfigHandler({}),
      relayEndpoint.mutation(
        "ScanControllerRegister2Mutation",
        ({ variables }) => {
          seen.push(variables.quickPick);
          return HttpResponse.json(signOutUserResponse());
        },
      ),
    );
    const user = await setupTest();
    await user.type(screen.getByRole("textbox"), SIGNOUT_USER + "{enter}");

    await waitFor(() => expect(seen).toEqual([false]));
  });
});

describe("KioskMain number pad", () => {
  async function setupNumberPadTest() {
    server.use(sessionConfigHandler({ interfaceMode: "touch" }));
    return await setupTest();
  }

  // Not vi.restoreAllMocks(): that would also undo this file's module-level
  // spies, which the rest of the file still asserts against.
  let userAgentSpy: { mockRestore: () => void } | null = null;
  function setUserAgent(ua: string) {
    userAgentSpy = vi.spyOn(navigator, "userAgent", "get").mockReturnValue(ua);
  }

  afterEach(() => {
    userAgentSpy?.mockRestore();
    userAgentSpy = null;
    Reflect.deleteProperty(navigator, "maxTouchPoints");
  });

  function tap(user: UserEventInstance, label: string) {
    return user.click(screen.getByRole("button", { name: label }));
  }

  async function openPad(user: UserEventInstance) {
    await tap(user, "Number pad");
    await waitFor(() =>
      expect(screen.getByText("Enter your SES ID")).toBeInTheDocument(),
    );
  }

  it("offers no number pad when auto detection finds a mouse and keyboard", async () => {
    await setupTest();
    expect(
      screen.queryByRole("button", { name: "Number pad" }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Submit" })).toBeInTheDocument();
  });

  it("offers no number pad in mouse and keyboard mode, even on an iPad", async () => {
    setUserAgent(
      "Mozilla/5.0 (iPad; CPU OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1",
    );
    server.use(sessionConfigHandler({ interfaceMode: "mouseKeyboard" }));
    const user = await setupTest();
    expect(
      screen.queryByRole("button", { name: "Number pad" }),
    ).not.toBeInTheDocument();

    await user.click(screen.getByRole("textbox"));
    expect(screen.queryByText("Enter your SES ID")).not.toBeInTheDocument();
  });

  it("detects an iPad as a touch kiosk in auto mode", async () => {
    // iPadOS asks for desktop sites by default, so its user agent claims to be
    // a Mac; only the touch points give it away.
    setUserAgent(
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15",
    );
    Object.defineProperty(navigator, "maxTouchPoints", {
      configurable: true,
      get: () => 5,
    });
    await setupTest();
    expect(
      screen.getByRole("button", { name: "Number pad" }),
    ).toBeInTheDocument();
  });

  it("opens the pad dialog from a tap on the member ID input", async () => {
    const user = await setupNumberPadTest();
    await user.click(screen.getByRole("textbox"));
    await waitFor(() =>
      expect(screen.getByText("Enter your SES ID")).toBeInTheDocument(),
    );
  });

  it("opens the pad dialog from the button beside the input", async () => {
    const user = await setupNumberPadTest();
    expect(
      screen.queryByRole("button", { name: "Submit" }),
    ).not.toBeInTheDocument();

    await openPad(user);
    expect(screen.getByRole("button", { name: "5" })).toBeInTheDocument();
  });

  it("types into the member ID input, then submits and closes on Confirm", async () => {
    const user = await setupNumberPadTest();
    await openPad(user);
    const textbox = screen.getByRole("textbox");

    for (const digit of FOUND_USER) {
      await tap(user, digit);
    }
    expect(textbox).toHaveValue(FOUND_USER);

    await tap(user, "Confirm");
    await waitFor(() =>
      expect(screen.queryByText("Enter your SES ID")).not.toBeInTheDocument(),
    );
    await waitFor(() =>
      expect(
        screen.getByText(
          FOUND_USER_RESPONSE.data.scanRegister2.period.person.firstName +
            " " +
            FOUND_USER_RESPONSE.data.scanRegister2.period.person.lastName,
        ),
      ).toBeInTheDocument(),
    );
    expect(textbox).toHaveValue("");
  });

  it("swaps the pad button for Submit once an ID has been entered", async () => {
    const user = await setupNumberPadTest();
    await openPad(user);
    await tap(user, "1");
    await tap(user, "Close");

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Submit" }),
      ).toBeInTheDocument(),
    );
    expect(
      screen.queryByRole("button", { name: "Number pad" }),
    ).not.toBeInTheDocument();
  });

  it("closes the pad when the entry is submitted from the input", async () => {
    const user = await setupNumberPadTest();
    await openPad(user);

    // What a barcode scanner does: keystrokes and a trailing newline. The
    // input is not focused behind the dialog, so the pad picks them up itself.
    await user.keyboard(FOUND_USER + "{Enter}");
    expect(screen.getByRole("textbox")).toHaveValue("");

    await waitFor(() =>
      expect(screen.queryByText("Enter your SES ID")).not.toBeInTheDocument(),
    );
    await waitFor(() =>
      expect(screen.getByText(/signed in at/)).toBeInTheDocument(),
    );
  });

  it("deletes the last digit", async () => {
    const user = await setupNumberPadTest();
    await openPad(user);
    const textbox = screen.getByRole("textbox");

    await tap(user, "1");
    await tap(user, "2");
    await tap(user, "Delete");
    expect(textbox).toHaveValue("1");

    // Deleting past the start is a no-op rather than an error.
    await tap(user, "Delete");
    await tap(user, "Delete");
    expect(textbox).toHaveValue("");
  });

  it("stops at the length of a member ID", async () => {
    const user = await setupNumberPadTest();
    await openPad(user);
    const textbox = screen.getByRole("textbox");

    for (const digit of "0123456789") {
      await tap(user, digit);
    }
    expect(textbox).toHaveValue("01234567");
  });

  it("keeps focus out of the member ID input so a touch keyboard stays hidden", async () => {
    const user = await setupNumberPadTest();
    const textbox = screen.getByRole("textbox");
    // The input takes focus from an effect, which can land a task after the
    // scan screen first renders.
    await waitFor(() => expect(document.activeElement).toBe(textbox));

    await openPad(user);
    expect(document.activeElement).not.toBe(textbox);

    await tap(user, "7");
    expect(textbox).toHaveValue("7");
    expect(document.activeElement).not.toBe(textbox);
  });

  it("takes typed digits and Backspace from a physical keyboard", async () => {
    const user = await setupNumberPadTest();
    await openPad(user);
    const textbox = screen.getByRole("textbox");

    await user.keyboard("12a3{Backspace}");
    expect(textbox).toHaveValue("12");
  });

  it("closes on Escape and gives focus back to the input", async () => {
    const user = await setupNumberPadTest();
    await openPad(user);
    const textbox = screen.getByRole("textbox");

    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(screen.queryByText("Enter your SES ID")).not.toBeInTheDocument(),
    );
    // Not straight away: on an iPad, a focus made while handling the tap that
    // closed the pad would bring the system keyboard up in its place.
    expect(document.activeElement).not.toBe(textbox);
    await waitFor(() => expect(document.activeElement).toBe(textbox), {
      timeout: 3_000,
    });
  });

  it("keeps the system keyboard hidden in touch mode", async () => {
    await setupNumberPadTest();
    expect(screen.getByRole("textbox")).toHaveAttribute("inputmode", "none");
  });

  it("asks for the number keyboard outside touch mode", async () => {
    await setupTest();
    const textbox = screen.getByRole("textbox");
    expect(textbox).toHaveAttribute("inputmode", "numeric");
    expect(textbox).toHaveAttribute("pattern", "[0-9]*");
  });

  it("cannot submit an empty entry", async () => {
    const user = await setupNumberPadTest();
    await openPad(user);

    expect(screen.getByRole("button", { name: "Confirm" })).toBeDisabled();
    expect(audioPlaySpy).not.toHaveBeenCalled();
  });
});

describe("KioskMain forgot-to-sign-out interstitial", () => {
  function longSignOutHandler() {
    return relayEndpoint.mutation("ScanControllerRegister2Mutation", () =>
      HttpResponse.json({
        data: {
          scanRegister2: {
            id: SIGNOUT_USER,
            state: "SIGN_OUT_PENDING",
            period: {
              id: "period-456",
              version: 1,
              // signed in well over 12 hours ago (unix seconds, as the API sends)
              startTime: Math.floor(Date.now() / 1000) - 60 * 60 * 30,
              endTime: null,
              person: {
                id: `person-${SIGNOUT_USER}`,
                firstName: "Jamie",
                lastName: "Smith",
              },
            },
            quickPick: null,
          },
        },
      }),
    );
  }

  it("shows the interstitial for a long session, then proceeds to categories", async () => {
    server.use(sessionConfigHandler({}), longSignOutHandler());
    const user = await setupTest();
    await user.type(screen.getByRole("textbox"), SIGNOUT_USER + "{enter}");

    await waitFor(() =>
      expect(
        screen.getByText("Did you forget to sign out?"),
      ).toBeInTheDocument(),
    );

    await user.click(screen.getByRole("button", { name: /Nope/i }));

    await waitFor(() =>
      expect(screen.getByText("Categories")).toBeInTheDocument(),
    );
  });

  it("is not shown for a short session", async () => {
    server.use(sessionConfigHandler({}), register2Handler());
    const user = await setupTest();
    await user.type(screen.getByRole("textbox"), SIGNOUT_USER + "{enter}");

    await waitFor(() =>
      expect(screen.getByText("Categories")).toBeInTheDocument(),
    );
    expect(
      screen.queryByText("Did you forget to sign out?"),
    ).not.toBeInTheDocument();
  });

  // Captures the signOut mutation vars so a test can inspect the times it sends.
  function captureSignOut(): { current: Record<string, number> | undefined } {
    const box: { current: Record<string, number> | undefined } = {
      current: undefined,
    };
    server.use(
      relayEndpoint.mutation(
        "ScanControllerSignOutMutation",
        ({ variables }) => {
          box.current = variables as Record<string, number>;
          return HttpResponse.json({
            data: {
              scanSignOut: {
                id: "period-456",
                version: 2,
                person: {
                  id: `person-${SIGNOUT_USER}`,
                  firstName: "Jamie",
                  lastName: "Smith",
                },
                startTime: variables.startTime,
                endTime: variables.endTime,
                category: { id: "RX2bfpU6ppvV", name: "AIIMS" },
              },
            },
          });
        },
      ),
    );
    return box;
  }

  async function pickAiimsCategory(user: UserEventInstance) {
    await waitFor(() =>
      expect(screen.getByText("Categories")).toBeInTheDocument(),
    );
    await user.click(screen.getByText("Training"));
    await user.click(screen.getByText("AIIMS"));
    await waitFor(() =>
      expect(screen.getByText("Confirm")).toBeInTheDocument(),
    );
  }

  // The member-ID form on the main screen also has an accessible name of
  // "Submit"; the confirm screen's is the one with visible text.
  async function clickConfirmSubmit(user: UserEventInstance) {
    const submit = screen
      .getAllByRole("button", { name: "Submit" })
      .find((b) => b.textContent?.trim() === "Submit");
    await user.click(submit!);
  }

  it("'Yeah' back-dates the sign-out to one hour after sign-in", async () => {
    server.use(sessionConfigHandler({}), longSignOutHandler());
    const signOut = captureSignOut();
    const user = await setupTest();
    await user.type(screen.getByRole("textbox"), SIGNOUT_USER + "{enter}");

    await waitFor(() =>
      expect(
        screen.getByText("Did you forget to sign out?"),
      ).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: /Yeah/i }));

    await pickAiimsCategory(user);
    // A ~1h period is under the 12h "long session" threshold, so Submit goes
    // straight through without the confirm dialog.
    await clickConfirmSubmit(user);

    await waitFor(() => expect(signOut.current).toBeDefined());
    const { startTime, endTime } = signOut.current!;
    expect(endTime - startTime).toBeGreaterThan(0);
    expect(endTime - startTime).toBeLessThan(2 * 60 * 60);
  });

  it("'Nope' keeps the sign-out time as now", async () => {
    server.use(sessionConfigHandler({}), longSignOutHandler());
    const signOut = captureSignOut();
    const user = await setupTest();
    await user.type(screen.getByRole("textbox"), SIGNOUT_USER + "{enter}");

    await waitFor(() =>
      expect(
        screen.getByText("Did you forget to sign out?"),
      ).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: /Nope/i }));

    await pickAiimsCategory(user);
    await clickConfirmSubmit(user);

    // ~30h period trips the long-session confirmation.
    await user.click(await screen.findByRole("button", { name: "Confirm" }));

    await waitFor(() => expect(signOut.current).toBeDefined());
    const { startTime, endTime } = signOut.current!;
    expect(endTime - startTime).toBeGreaterThan(20 * 60 * 60);
  });
});

describe("KioskMain status screen", () => {
  it("shows an error state instead of crashing when a field fails to resolve", async () => {
    // Same shape as a dangling person reference: data present, but one field
    // errored. @throwOnFieldError on LivePeriodsSnapshotQuery turns this into
    // a rejected fetchQuery promise, which LivePeriodsProvider turns into
    // `error` rather than a null silently reaching StatusCurrentDisplay.
    vi.spyOn(console, "error").mockImplementation(() => {});
    server.use(
      sessionConfigHandler({ status: true }),
      relayEndpoint.query("LivePeriodsSnapshotQuery", () =>
        HttpResponse.json({
          data: {
            session: {
              location: {
                periods: {
                  edges: [
                    {
                      node: {
                        id: "period-1",
                        version: 1,
                        startTime: Math.floor(Date.now() / 1000),
                        guestName: null,
                        person: null,
                      },
                    },
                  ],
                },
              },
            },
          },
          errors: [
            {
              message: "Person with ID abc123 missing",
              path: [
                "session",
                "location",
                "periods",
                "edges",
                0,
                "node",
                "person",
              ],
            },
          ],
        }),
      ),
    );

    render(<KioskMain />);

    await waitFor(() =>
      expect(
        screen.getByText(/Couldn't load the signed-in list/),
      ).toBeInTheDocument(),
    );
  });

  it("recovers when 'Try again' is clicked once the query starts succeeding", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    server.use(
      sessionConfigHandler({ status: true }),
      relayEndpoint.query("LivePeriodsSnapshotQuery", () =>
        HttpResponse.json({
          data: {
            session: {
              location: {
                periods: {
                  edges: [
                    {
                      node: {
                        id: "period-1",
                        version: 1,
                        startTime: Math.floor(Date.now() / 1000),
                        guestName: null,
                        person: null,
                      },
                    },
                  ],
                },
              },
            },
          },
          errors: [
            {
              message: "Person with ID abc123 missing",
              path: [
                "session",
                "location",
                "periods",
                "edges",
                0,
                "node",
                "person",
              ],
            },
          ],
        }),
      ),
    );

    render(<KioskMain />);

    await waitFor(() =>
      expect(
        screen.getByText(/Couldn't load the signed-in list/),
      ).toBeInTheDocument(),
    );

    // Whatever caused the field error is now fixed server-side.
    server.use(
      relayEndpoint.query("LivePeriodsSnapshotQuery", () =>
        HttpResponse.json({
          data: {
            session: {
              location: {
                periods: {
                  edges: [
                    {
                      node: {
                        id: "period-1",
                        version: 1,
                        startTime: Math.floor(Date.now() / 1000),
                        guestName: "Random Guy",
                        person: null,
                      },
                    },
                  ],
                },
              },
            },
          },
        }),
      ),
    );

    const user = UserEvent.setup();
    await user.click(screen.getByRole("button", { name: "Try again" }));

    // The regression this guards: "Try again" must actually restart
    // LivePeriodsProvider's fetch cycle (retry() bumps its retryGeneration),
    // not just redisplay whatever `error` was last set to.
    await waitFor(() =>
      expect(screen.getByText("1 member signed in")).toBeInTheDocument(),
    );
    expect(screen.getByText("Random Guy (Guest)")).toBeInTheDocument();
  });
});

describe("KioskMain signed-in status", () => {
  const nowSecs = Math.floor(Date.now() / 1000);
  const TWO_PEOPLE = [
    {
      id: "period-1",
      version: 1,
      startTime: nowSecs - 60 * 60,
      guestName: null,
      person: { id: "person-1", firstName: "Alice", lastName: "Anderson" },
    },
    {
      id: "period-2",
      version: 1,
      startTime: nowSecs - 60 * 30,
      guestName: "Jamie Visitor",
      person: null,
    },
  ];

  it("hides the button when the kiosk does not have it enabled", async () => {
    server.use(sessionConfigHandler({}));
    await setupTest();

    expect(
      screen.queryByRole("button", { name: "Who's here" }),
    ).not.toBeInTheDocument();
  });

  it("lists members and guests with the total when the button is pressed", async () => {
    server.use(
      sessionConfigHandler({ signedInStatus: true }),
      snapshotHandler(TWO_PEOPLE),
    );
    const user = await setupTest();

    await user.click(screen.getByRole("button", { name: "Who's here" }));

    await waitFor(() =>
      expect(screen.getByText("Alice Anderson")).toBeInTheDocument(),
    );
    expect(screen.getByText("Jamie Visitor (Guest)")).toBeInTheDocument();
    expect(screen.getByText("2 signed in")).toBeInTheDocument();
  });

  it("says so when nobody is signed in", async () => {
    server.use(
      sessionConfigHandler({ signedInStatus: true }),
      snapshotHandler([]),
    );
    const user = await setupTest();

    await user.click(screen.getByRole("button", { name: "Who's here" }));

    await waitFor(() =>
      expect(
        screen.getByText("Nobody is currently signed in here."),
      ).toBeInTheDocument(),
    );
  });

  it("gets out of the way when someone scans while it is open", async () => {
    // The list holds no scan focus lease precisely so this works: the member ID
    // input keeps focus underneath it, and the scan closes the list rather than
    // having its result hidden behind it. Typed with `keyboard` rather than
    // `type` so it goes wherever focus actually is — pressing the button leaves
    // focus on the button unless opening the list hands it straight back, and a
    // scanner's keystrokes have no say in the matter.
    server.use(
      sessionConfigHandler({ signedInStatus: true }),
      snapshotHandler(TWO_PEOPLE),
      register2Handler(),
    );
    const user = await setupTest();

    await user.click(screen.getByRole("button", { name: "Who's here" }));
    await waitFor(() =>
      expect(screen.getByText("Alice Anderson")).toBeInTheDocument(),
    );
    expect(screen.getByRole("textbox")).toHaveFocus();

    await user.keyboard(FOUND_USER + "{Enter}");

    await waitFor(() =>
      expect(screen.queryByText("Alice Anderson")).not.toBeInTheDocument(),
    );
    expect(await screen.findByText("Random Guy")).toBeInTheDocument();
  });
});

describe("KioskMain signed-in status inline panel", () => {
  const nowSecs = Math.floor(Date.now() / 1000);
  const TWO_PEOPLE = [
    {
      id: "period-1",
      version: 1,
      startTime: nowSecs - 60 * 60,
      guestName: null,
      person: { id: "person-1", firstName: "Alice", lastName: "Anderson" },
    },
    {
      id: "period-2",
      version: 1,
      startTime: nowSecs - 60 * 30,
      guestName: "Jamie Visitor",
      person: null,
    },
  ];

  it("is not shown when the kiosk does not have it enabled", async () => {
    server.use(sessionConfigHandler({}));
    await setupTest();

    expect(screen.queryByText("Currently signed in")).not.toBeInTheDocument();
  });

  it("shows the list on the main screen without pressing anything, and hides the button", async () => {
    server.use(
      sessionConfigHandler({ signedInStatusInline: true }),
      snapshotHandler(TWO_PEOPLE),
    );
    await setupTest();

    await waitFor(() =>
      expect(screen.getByText("Alice Anderson")).toBeInTheDocument(),
    );
    expect(screen.getByText("Jamie Visitor (Guest)")).toBeInTheDocument();
    expect(screen.getByText("2 signed in")).toBeInTheDocument();
    // Redundant next to a list that's already on screen.
    expect(
      screen.queryByRole("button", { name: "Who's here" }),
    ).not.toBeInTheDocument();
  });

  it("prefers inline over the button when both flags are somehow set", async () => {
    server.use(
      sessionConfigHandler({
        signedInStatus: true,
        signedInStatusInline: true,
      }),
      snapshotHandler([]),
    );
    await setupTest();

    await waitFor(() =>
      expect(screen.getByText("Currently signed in")).toBeInTheDocument(),
    );
    expect(
      screen.queryByRole("button", { name: "Who's here" }),
    ).not.toBeInTheDocument();
  });

  it("says so when nobody is signed in", async () => {
    server.use(
      sessionConfigHandler({ signedInStatusInline: true }),
      snapshotHandler([]),
    );
    await setupTest();

    await waitFor(() =>
      expect(screen.getByText("Nobody signed in here.")).toBeInTheDocument(),
    );
  });

  it("does not steal focus from the member ID input", async () => {
    server.use(
      sessionConfigHandler({ signedInStatusInline: true }),
      snapshotHandler(TWO_PEOPLE),
    );
    await setupTest();

    await waitFor(() =>
      expect(screen.getByText("Alice Anderson")).toBeInTheDocument(),
    );
    expect(screen.getByRole("textbox")).toHaveFocus();
  });
});

describe("KioskMain signed-in list sign out", () => {
  const nowSecs = Math.floor(Date.now() / 1000);
  const TWO_PEOPLE = [
    {
      id: "period-1",
      version: 1,
      startTime: nowSecs - 60 * 60,
      guestName: null,
      person: { id: "person-1", firstName: "Alice", lastName: "Anderson" },
    },
    {
      id: "period-2",
      version: 1,
      startTime: nowSecs - 60 * 30,
      guestName: "Jamie Visitor",
      person: null,
    },
  ];
  const TWO_MEMBERS = [
    TWO_PEOPLE[0],
    {
      id: "period-3",
      version: 1,
      startTime: nowSecs - 60 * 20,
      guestName: null,
      person: { id: "person-3", firstName: "Bob", lastName: "Brown" },
    },
  ];
  const SIGN_OUT_CONFIG = {
    signedInStatusInline: true,
    signedInStatusSignOut: true,
  };

  type StartVariables = { id: string; quickPick: boolean };

  // The people behind each row, so the server's answer matches the row pressed.
  const PEOPLE_BY_PERIOD: Record<
    string,
    { firstName: string; lastName: string }
  > = {
    "period-1": { firstName: "Alice", lastName: "Anderson" },
    "period-3": { firstName: "Bob", lastName: "Brown" },
  };

  // What a list press asks the server: start the sign-out for this exact period.
  // It answers with the same SIGN_OUT_PENDING result a scan of a signed-in member
  // does, so the kiosk carries on to the category screen as it would after a scan.
  function startSignOutHandler(
    onCall?: (variables: StartVariables) => void,
    gate?: Promise<void>,
  ) {
    return relayEndpoint.mutation(
      "ScanControllerStartSignOutMutation",
      async ({ variables }) => {
        onCall?.(variables as StartVariables);
        await gate;
        const id = (variables as StartVariables).id;
        const result = signOutUserResponse(null).data.scanRegister2;
        return HttpResponse.json({
          data: {
            scanStartSignOut: {
              ...result,
              period: {
                ...result.period,
                startTime: nowSecs - 60 * 60,
                id,
                person: { id: `person-of-${id}`, ...PEOPLE_BY_PERIOD[id] },
              },
            },
          },
        });
      },
    );
  }

  // The server refuses a stale row with a code and a message written for the kiosk.
  function refusedStartSignOutHandler(code: string, message: string) {
    return relayEndpoint.mutation("ScanControllerStartSignOutMutation", () =>
      HttpResponse.json({
        data: null,
        errors: [{ message, extensions: { code } }],
      }),
    );
  }

  // A scan toggles sign-in/out, so a list press must never reach it.
  function register2Spy(registered: string[]) {
    return relayEndpoint.mutation(
      "ScanControllerRegister2Mutation",
      ({ variables }) => {
        registered.push(variables.memberNumber);
        return HttpResponse.json(signOutUserResponse(null));
      },
    );
  }

  async function pressSignOutOf(
    user: UserEventInstance,
    name = "Alice Anderson",
  ) {
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: `Sign out ${name}` }),
      ).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: `Sign out ${name}` }));
  }

  // Every scan screen stays mounted off to the side (inert) until its turn, so
  // "Categories is in the document" is true from the start. Wait for it to be the
  // screen that is actually on.
  async function expectCategoryScreen() {
    await waitFor(() =>
      expect(screen.getByText("Categories").closest("[inert]")).toBeNull(),
    );
  }

  // Finish the sign-out the list started: pick a category, confirm, and report
  // what the kiosk finally submitted.
  function signOutSpy(submitted: Array<Record<string, unknown>>) {
    return relayEndpoint.mutation(
      "ScanControllerSignOutMutation",
      ({ variables }) => {
        submitted.push(variables as Record<string, unknown>);
        const id = (variables as { id: string }).id;
        return HttpResponse.json({
          data: {
            scanSignOut: {
              id,
              version: 2,
              person: { id: `person-of-${id}`, ...PEOPLE_BY_PERIOD[id] },
              startTime: (variables as { startTime: number }).startTime,
              endTime: (variables as { endTime: number }).endTime,
              category: { id: "RX2bfpU6ppvV", name: "AIIMS" },
            },
          },
        });
      },
    );
  }

  async function finishSignOut(user: UserEventInstance) {
    await expectCategoryScreen();
    await user.click(screen.getByText("Training"));
    await user.click(screen.getByText("AIIMS"));
    await waitFor(() =>
      expect(screen.getByText("Confirm")).toBeInTheDocument(),
    );
    const confirm = await waitFor(() => {
      const found = screen
        .getAllByRole("button", { name: "Submit" })
        .find((b) => b.textContent?.trim() === "Submit");
      expect(found).toBeDefined();
      return found!;
    });
    await user.click(confirm);
  }

  it("shows no Sign out buttons on the plain always-visible list", async () => {
    server.use(
      sessionConfigHandler({ signedInStatusInline: true }),
      snapshotHandler(TWO_PEOPLE),
    );
    await setupTest();

    await waitFor(() =>
      expect(screen.getByText("Alice Anderson")).toBeInTheDocument(),
    );
    expect(
      screen.queryByRole("button", { name: /^Sign out/ }),
    ).not.toBeInTheDocument();
  });

  it("shows nothing for sign out without the always-visible list", async () => {
    server.use(
      sessionConfigHandler({ signedInStatusSignOut: true }),
      snapshotHandler(TWO_PEOPLE),
    );
    await setupTest();

    expect(screen.queryByText("Currently signed in")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /^Sign out/ }),
    ).not.toBeInTheDocument();
  });

  it("offers Sign out for members but not guests", async () => {
    server.use(
      sessionConfigHandler(SIGN_OUT_CONFIG),
      snapshotHandler(TWO_PEOPLE),
    );
    await setupTest();

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Sign out Alice Anderson" }),
      ).toBeInTheDocument(),
    );
    // Guests have no card to scan and keep their own dialog.
    expect(screen.getByText("Jamie Visitor (Guest)")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /Sign out Jamie/ }),
    ).not.toBeInTheDocument();
  });

  it("starts the normal sign-out for the row that was pressed, without a scan", async () => {
    const started: StartVariables[] = [];
    const registered: string[] = [];
    server.use(
      sessionConfigHandler(SIGN_OUT_CONFIG),
      snapshotHandler(TWO_PEOPLE),
      startSignOutHandler((variables) => started.push(variables)),
      register2Spy(registered),
    );
    const user = await setupTest();

    await pressSignOutOf(user);

    // Not signed out outright: the member still picks a category and confirms.
    await expectCategoryScreen();
    expect(started).toEqual([{ id: "period-1", quickPick: false }]);
    // The scan path toggles sign-in/out, so the list press must never use it.
    expect(registered).toEqual([]);
  });

  it("asks for the quick pick when the kiosk uses it", async () => {
    const started: StartVariables[] = [];
    server.use(
      sessionConfigHandler({ ...SIGN_OUT_CONFIG, quickPickCategories: true }),
      snapshotHandler(TWO_PEOPLE),
      startSignOutHandler((variables) => started.push(variables)),
    );
    const user = await setupTest();

    await pressSignOutOf(user);

    await waitFor(() => expect(started).toHaveLength(1));
    expect(started[0]).toEqual({ id: "period-1", quickPick: true });
  });

  it("hands focus back to the member ID input after the press", async () => {
    server.use(
      sessionConfigHandler(SIGN_OUT_CONFIG),
      snapshotHandler(TWO_PEOPLE),
      startSignOutHandler(),
    );
    const user = await setupTest();

    await pressSignOutOf(user);

    expect(screen.getByRole("textbox")).toHaveFocus();
  });

  it("shows who it is fetching for, and greys the buttons out, while it starts", async () => {
    let release: () => void = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    server.use(
      sessionConfigHandler(SIGN_OUT_CONFIG),
      snapshotHandler(TWO_MEMBERS),
      startSignOutHandler(undefined, gate),
    );
    const user = await setupTest();

    await pressSignOutOf(user);

    await waitFor(() =>
      expect(
        screen.getByText(/Fetching information for\s+Alice Anderson/),
      ).toBeInTheDocument(),
    );
    expect(
      screen.getByRole("button", { name: "Sign out Bob Brown" }),
    ).toBeDisabled();

    release();
    await expectCategoryScreen();
  });

  it("says so when the row is already signed out, and goes no further", async () => {
    const registered: string[] = [];
    vi.spyOn(console, "error").mockImplementation(() => {});
    server.use(
      sessionConfigHandler(SIGN_OUT_CONFIG),
      snapshotHandler(TWO_PEOPLE),
      refusedStartSignOutHandler("CONFLICT", "Already signed out"),
      register2Spy(registered),
    );
    const user = await setupTest();

    await pressSignOutOf(user);

    await waitFor(() =>
      expect(
        screen.getByText(
          "could not sign out Alice Anderson: Already signed out",
        ),
      ).toBeInTheDocument(),
    );
    // A stale row can never become a fresh sign-in.
    expect(registered).toEqual([]);
    // The category screen stays parked off to the side (inert) rather than coming on.
    expect(screen.getByText("Categories").closest("[inert]")).not.toBeNull();
  });

  it.each([
    ["CONFLICT", "Already signed out"],
    ["NOT_FOUND", "That sign-in no longer exists"],
  ])(
    "fetches the list again after a %s refusal, so the dead row goes",
    async (code, message) => {
      let snapshots = 0;
      vi.spyOn(console, "error").mockImplementation(() => {});
      server.use(
        sessionConfigHandler(SIGN_OUT_CONFIG),
        relayEndpoint.query("LivePeriodsSnapshotQuery", () => {
          snapshots += 1;
          return HttpResponse.json({
            data: {
              session: {
                location: {
                  periods: {
                    // Alice is gone by the second fetch.
                    edges: (snapshots === 1
                      ? TWO_MEMBERS
                      : [TWO_MEMBERS[1]]
                    ).map((node) => ({ node })),
                  },
                },
              },
            },
          });
        }),
        refusedStartSignOutHandler(code, message),
      );
      const user = await setupTest();

      await pressSignOutOf(user);

      await waitFor(() => expect(snapshots).toBeGreaterThan(1));
      await waitFor(() =>
        expect(screen.queryByText("Alice Anderson")).not.toBeInTheDocument(),
      );
      expect(screen.getByText("Bob Brown")).toBeInTheDocument();
    },
  );

  it("says so when the row's sign-in no longer exists", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    server.use(
      sessionConfigHandler(SIGN_OUT_CONFIG),
      snapshotHandler(TWO_PEOPLE),
      refusedStartSignOutHandler("NOT_FOUND", "That sign-in no longer exists"),
    );
    const user = await setupTest();

    await pressSignOutOf(user);

    await waitFor(() =>
      expect(
        screen.getByText(
          "could not sign out Alice Anderson: That sign-in no longer exists",
        ),
      ).toBeInTheDocument(),
    );
  });

  it("reports a failed request and lets the operator try again", async () => {
    let calls = 0;
    vi.spyOn(console, "error").mockImplementation(() => {});
    server.use(
      sessionConfigHandler(SIGN_OUT_CONFIG),
      snapshotHandler(TWO_PEOPLE),
      relayEndpoint.mutation(
        "ScanControllerStartSignOutMutation",
        ({ variables }) => {
          calls += 1;
          if (calls === 1) {
            return HttpResponse.json(
              { errors: [{ message: "boom" }] },
              { status: 500 },
            );
          }
          const result = signOutUserResponse(null).data.scanRegister2;
          return HttpResponse.json({
            data: {
              scanStartSignOut: {
                ...result,
                period: {
                  ...result.period,
                  startTime: nowSecs - 60 * 60,
                  id: (variables as StartVariables).id,
                  person: { id: "person-1", ...PEOPLE_BY_PERIOD["period-1"] },
                },
              },
            },
          });
        },
      ),
    );
    const user = await setupTest();

    await pressSignOutOf(user);

    await waitFor(() =>
      expect(
        screen.getByText(
          "network issue while starting the sign-out for Alice Anderson",
        ),
      ).toBeInTheDocument(),
    );
    // Nothing was started, so the button comes back and works.
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Sign out Alice Anderson" }),
      ).toBeEnabled(),
    );
    await pressSignOutOf(user);
    await expectCategoryScreen();
    expect(calls).toBe(2);
  });

  it("ignores a second press while the first is still starting", async () => {
    const started: StartVariables[] = [];
    let release: () => void = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    server.use(
      sessionConfigHandler(SIGN_OUT_CONFIG),
      snapshotHandler(TWO_MEMBERS),
      startSignOutHandler((variables) => started.push(variables), gate),
    );
    const user = await setupTest();
    await waitFor(() =>
      expect(screen.getByText("Bob Brown")).toBeInTheDocument(),
    );
    const alice = screen.getByRole("button", {
      name: "Sign out Alice Anderson",
    });
    const bob = screen.getByRole("button", { name: "Sign out Bob Brown" });

    await user.click(alice);
    await user.click(alice);
    await user.click(bob);

    expect(started).toEqual([{ id: "period-1", quickPick: false }]);
    release();
    await expectCategoryScreen();
    expect(started).toHaveLength(1);
  });

  it("does not run a card scanned while the sign-out is starting", async () => {
    const registered: string[] = [];
    let release: () => void = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    server.use(
      sessionConfigHandler(SIGN_OUT_CONFIG),
      snapshotHandler(TWO_PEOPLE),
      startSignOutHandler(undefined, gate),
      register2Spy(registered),
    );
    const user = await setupTest();

    await pressSignOutOf(user);
    await user.keyboard(FOUND_USER + "{Enter}");

    expect(registered).toEqual([]);
    release();
    await expectCategoryScreen();
    expect(registered).toEqual([]);
  });

  it("ignores a second press in the same moment as the first", async () => {
    const started: StartVariables[] = [];
    let release: () => void = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    server.use(
      sessionConfigHandler(SIGN_OUT_CONFIG),
      snapshotHandler(TWO_PEOPLE),
      startSignOutHandler((variables) => started.push(variables), gate),
    );
    await setupTest();
    const alice = await screen.findByRole("button", {
      name: "Sign out Alice Anderson",
    });

    // Both presses land before React re-renders, so neither the greyed-out
    // buttons nor the transaction state can have caught the first yet.
    act(() => {
      alice.click();
      alice.click();
    });

    // A second request, had one been sent, would reach the server before the
    // first one is let go, so it is already counted once the sign-out goes on.
    release();
    await expectCategoryScreen();
    expect(started).toEqual([{ id: "period-1", quickPick: false }]);
  });

  it("submits the sign-out for the row that was pressed, then lets another be started straight away", async () => {
    const started: StartVariables[] = [];
    const submitted: Array<Record<string, unknown>> = [];
    server.use(
      sessionConfigHandler(SIGN_OUT_CONFIG),
      snapshotHandler(TWO_MEMBERS),
      startSignOutHandler((variables) => started.push(variables)),
      signOutSpy(submitted),
    );
    const user = await setupTest();

    await pressSignOutOf(user, "Alice Anderson");
    await finishSignOut(user);

    // What the kiosk finally submitted is the pressed row's period, not a guess.
    await waitFor(() => expect(submitted).toHaveLength(1));
    expect(submitted[0]).toMatchObject({ id: "period-1" });

    // The finished sign-out must not leave the other rows locked until it ages
    // out of the log a minute later.
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Sign out Bob Brown" }),
      ).toBeEnabled(),
    );
    await pressSignOutOf(user, "Bob Brown");
    await expectCategoryScreen();
    expect(started).toEqual([
      { id: "period-1", quickPick: false },
      { id: "period-3", quickPick: false },
    ]);
  });
});

import "@testing-library/jest-dom/vitest";
import { vi, describe, it, expect, beforeEach } from "vitest";
import UserEvent from "@testing-library/user-event";
import type { UserEvent as UserEventInstance } from "@testing-library/user-event";
import { render, screen, waitFor } from "@testing-library/react";
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
          location: {
            id: "mockLocationId",
            name: "mockLocation",
          },
        },
      },
    });
  });
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

const graphqlHandlers = [sessionConfigHandler({}), register2Handler()];

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
    expect(document.documentElement).toHaveAttribute("data-theme", "light");
  });

  it("pins dark when the theme is 'dark'", async () => {
    server.use(sessionConfigHandler({ theme: "dark" }));
    await setupTest();
    expect(document.documentElement).toHaveAttribute("data-theme", "dark");
  });

  it("leaves the theme unpinned when the theme is 'auto'", async () => {
    server.use(sessionConfigHandler({ theme: "auto" }));
    await setupTest();
    expect(document.documentElement).not.toHaveAttribute("data-theme");
  });

  it("pins light for an unrecognised theme value", async () => {
    server.use(sessionConfigHandler({ theme: "solarized" }));
    await setupTest();
    expect(document.documentElement).toHaveAttribute("data-theme", "light");
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
    server.use(
      sessionConfigHandler({ easyTimeEntry: true }),
      longSignOutHandler(),
    );
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
    server.use(
      sessionConfigHandler({ easyTimeEntry: true }),
      longSignOutHandler(),
    );
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
  it("shows the error fallback instead of crashing when a field fails to resolve", async () => {
    // Same shape as a dangling person reference: data present, but one field
    // errored. @throwOnFieldError turns this into a thrown error the boundary
    // catches, instead of a null silently reaching StatusCurrentDisplay.
    vi.spyOn(console, "error").mockImplementation(() => {});
    server.use(
      sessionConfigHandler({ status: true }),
      relayEndpoint.query("StatusQuery", () =>
        HttpResponse.json({
          data: {
            session: {
              location: {
                periods: {
                  edges: [
                    {
                      node: {
                        id: "period-1",
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
      expect(screen.getByText("Something went wrong")).toBeInTheDocument(),
    );
  });

  it("recovers when 'Try again' is clicked once the query starts succeeding", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    server.use(
      sessionConfigHandler({ status: true }),
      relayEndpoint.query("StatusQuery", () =>
        HttpResponse.json({
          data: {
            session: {
              location: {
                periods: {
                  edges: [
                    {
                      node: {
                        id: "period-1",
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
      expect(screen.getByText("Something went wrong")).toBeInTheDocument(),
    );

    // Whatever caused the field error is now fixed server-side.
    server.use(
      relayEndpoint.query("StatusQuery", () =>
        HttpResponse.json({
          data: {
            session: {
              location: {
                periods: {
                  edges: [
                    {
                      node: {
                        id: "period-1",
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

    // This is the actual regression test for the fetchKey fix: if "Try
    // again" only invalidated the store (the pre-fetchKey behaviour), the
    // useLazyLoadQuery cache entry still holds the original thrown error and
    // this would keep showing "Something went wrong" forever, even though
    // the server would now respond successfully.
    await waitFor(() =>
      expect(screen.getByText("1 member signed in")).toBeInTheDocument(),
    );
    expect(screen.getByText("Random Guy (Guest)")).toBeInTheDocument();
  });
});

describe("KioskMain signed-in status", () => {
  function statusDialogHandler(
    nodes: Array<{
      id: string;
      startTime: number;
      guestName: string | null;
      person: { id: string; firstName: string; lastName: string } | null;
    }>,
  ) {
    return relayEndpoint.query("ScanStatusDialogQuery", () =>
      HttpResponse.json({
        data: {
          session: {
            location: { periods: { edges: nodes.map((node) => ({ node })) } },
          },
        },
      }),
    );
  }

  const nowSecs = Math.floor(Date.now() / 1000);
  const TWO_PEOPLE = [
    {
      id: "period-1",
      startTime: nowSecs - 60 * 60,
      guestName: null,
      person: { id: "person-1", firstName: "Alice", lastName: "Anderson" },
    },
    {
      id: "period-2",
      startTime: nowSecs - 60 * 30,
      guestName: "Jamie Visitor",
      person: null,
    },
  ];

  it("hides the button when the kiosk does not have it enabled", async () => {
    server.use(sessionConfigHandler({}));
    await setupTest();

    expect(
      screen.queryByRole("button", { name: "Who's signed in" }),
    ).not.toBeInTheDocument();
  });

  it("lists members and guests with the total when the button is pressed", async () => {
    server.use(
      sessionConfigHandler({ signedInStatus: true }),
      statusDialogHandler(TWO_PEOPLE),
    );
    const user = await setupTest();

    await user.click(screen.getByRole("button", { name: "Who's signed in" }));

    await waitFor(() =>
      expect(screen.getByText("Alice Anderson")).toBeInTheDocument(),
    );
    expect(screen.getByText("Jamie Visitor (Guest)")).toBeInTheDocument();
    expect(screen.getByText("2 signed in")).toBeInTheDocument();
  });

  it("says so when nobody is signed in", async () => {
    server.use(
      sessionConfigHandler({ signedInStatus: true }),
      statusDialogHandler([]),
    );
    const user = await setupTest();

    await user.click(screen.getByRole("button", { name: "Who's signed in" }));

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
      statusDialogHandler(TWO_PEOPLE),
      register2Handler(),
    );
    const user = await setupTest();

    await user.click(screen.getByRole("button", { name: "Who's signed in" }));
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
  function panelHandler(
    nodes: Array<{
      id: string;
      startTime: number;
      guestName: string | null;
      person: { id: string; firstName: string; lastName: string } | null;
    }>,
  ) {
    return relayEndpoint.query("ScanSignedInPanelQuery", () =>
      HttpResponse.json({
        data: {
          session: {
            location: { periods: { edges: nodes.map((node) => ({ node })) } },
          },
        },
      }),
    );
  }

  const nowSecs = Math.floor(Date.now() / 1000);
  const TWO_PEOPLE = [
    {
      id: "period-1",
      startTime: nowSecs - 60 * 60,
      guestName: null,
      person: { id: "person-1", firstName: "Alice", lastName: "Anderson" },
    },
    {
      id: "period-2",
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
      panelHandler(TWO_PEOPLE),
    );
    await setupTest();

    await waitFor(() =>
      expect(screen.getByText("Alice Anderson")).toBeInTheDocument(),
    );
    expect(screen.getByText("Jamie Visitor (Guest)")).toBeInTheDocument();
    expect(screen.getByText("2 signed in")).toBeInTheDocument();
    // Redundant next to a list that's already on screen.
    expect(
      screen.queryByRole("button", { name: "Who's signed in" }),
    ).not.toBeInTheDocument();
  });

  it("prefers inline over the button when both flags are somehow set", async () => {
    server.use(
      sessionConfigHandler({
        signedInStatus: true,
        signedInStatusInline: true,
      }),
      panelHandler([]),
    );
    await setupTest();

    await waitFor(() =>
      expect(screen.getByText("Currently signed in")).toBeInTheDocument(),
    );
    expect(
      screen.queryByRole("button", { name: "Who's signed in" }),
    ).not.toBeInTheDocument();
  });

  it("says so when nobody is signed in", async () => {
    server.use(
      sessionConfigHandler({ signedInStatusInline: true }),
      panelHandler([]),
    );
    await setupTest();

    await waitFor(() =>
      expect(screen.getByText("Nobody signed in here.")).toBeInTheDocument(),
    );
  });

  it("does not steal focus from the member ID input", async () => {
    server.use(
      sessionConfigHandler({ signedInStatusInline: true }),
      panelHandler(TWO_PEOPLE),
    );
    await setupTest();

    await waitFor(() =>
      expect(screen.getByText("Alice Anderson")).toBeInTheDocument(),
    );
    expect(screen.getByRole("textbox")).toHaveFocus();
  });
});

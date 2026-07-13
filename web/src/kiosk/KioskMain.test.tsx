import "@testing-library/jest-dom/vitest";
import { vi, describe, it, expect, beforeEach } from "vitest";
import UserEvent from "@testing-library/user-event";
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
    },
  },
};
const SETTINGS = {
  scanAuthToken: "mock-token",
  scanAuthTokenIssuedAt: new Date().getTime(),
};

const relayUrl = getGraphQLEndpoint();
const relayEndpoint = graphql.link(relayUrl);

const graphqlHandlers = [
  relayEndpoint.query("KioskTokenSessionFetcherQuery", () => {
    return HttpResponse.json({
      data: {
        refresh_token: "not-a-refreshed-token",
        session: {
          id: "mockId",
          name: "mockName",
          config: {},
          location: {
            id: "mockLocationId",
            name: "mockLocation",
          },
        },
      },
    });
  }),
  relayEndpoint.mutation("ScanControllerRegister2Mutation", ({ variables }) => {
    const { memberNumber } = variables;
    if (memberNumber !== FOUND_USER) {
      return HttpResponse.json({
        data: {
          scanRegister2: {
            id: memberNumber,
            state: "NOT_FOUND",
            period: null,
          },
        },
      });
    }
    return HttpResponse.json(FOUND_USER_RESPONSE);
  }),
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
    await user.type(screen.getByRole("textbox"), FOUND_USER + "{enter}");
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
});

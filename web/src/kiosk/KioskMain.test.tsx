import "@testing-library/jest-dom/vitest";
import { vi, describe, it, expect } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { getGraphQLEndpoint } from "../lib/api";
import { beforeAll, afterEach, afterAll } from "vitest";
import { setupServer } from "msw/node";
import { graphql, HttpResponse } from "msw";
import KioskMain from "./KioskMain";

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
];

const server = setupServer(...graphqlHandlers);

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

describe("KioskMain", () => {
  it("renders the main screen given a valid session", async () => {
    const settings = {
      scanAuthToken: "fun-token",
      scanAuthTokenIssuedAt: new Date().getTime(),
    };

    const getItemSpy = vi.spyOn(localStorage, "getItem");
    getItemSpy.mockReturnValue(JSON.stringify(settings));

    render(<KioskMain />);
    expect(getItemSpy).toHaveBeenCalledOnce();
    await waitFor(() =>
      expect(
        screen.getByText("Please enter or scan your SES ID"),
      ).toBeInTheDocument(),
    );
  });
});

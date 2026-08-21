import {
  describe,
  it,
  expect,
  vi,
  beforeAll,
  afterEach,
  afterAll,
} from "vitest";
import { setupServer } from "msw/node";
import { graphql, HttpResponse } from "msw";
import type { RequestParameters } from "relay-runtime";

import { getGraphQLEndpoint } from "./api";
import { fetchGraphQL } from "./graphql";
import { takeRecentFieldErrorMessages } from "./relayFieldLogger";

const relayEndpoint = graphql.link(getGraphQLEndpoint());
const server = setupServer();

beforeAll(() => server.listen());
afterEach(() => {
  server.resetHandlers();
  takeRecentFieldErrorMessages();
  vi.restoreAllMocks();
});
afterAll(() => server.close());

function queryRequest(name: string): RequestParameters {
  return {
    id: null,
    cacheID: name,
    text: `query ${name} { location(id: "x") { id } }`,
    name,
    operationKind: "query",
    metadata: {},
  };
}

describe("fetchGraphQL error capture", () => {
  it("buffers and logs a query response's errors, without throwing", async () => {
    server.use(
      relayEndpoint.query("SomeQuery", () =>
        HttpResponse.json({
          data: { location: { id: "x" } },
          errors: [
            {
              message: "Category with ID abc123 missing",
              path: ["location", "periodSummaryByCategory", 33, "category"],
            },
          ],
        }),
      ),
    );
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    const result = await fetchGraphQL(
      null,
      queryRequest("SomeQuery"),
      {},
      () => {},
    );

    expect(result.data).toEqual({ location: { id: "x" } });
    expect(takeRecentFieldErrorMessages()).toEqual([
      "Category with ID abc123 missing",
    ]);
    expect(consoleSpy).toHaveBeenCalledWith(
      "[graphql-error] SomeQuery:",
      result.errors,
    );
  });

  it("buffers every message from a multi-error response", async () => {
    server.use(
      relayEndpoint.query("SomeQuery", () =>
        HttpResponse.json({
          data: null,
          errors: [{ message: "first" }, { message: "second" }],
        }),
      ),
    );
    vi.spyOn(console, "error").mockImplementation(() => {});

    await fetchGraphQL(null, queryRequest("SomeQuery"), {}, () => {});

    expect(takeRecentFieldErrorMessages()).toEqual(["first", "second"]);
  });

  it("does nothing when the response has no errors", async () => {
    server.use(
      relayEndpoint.query("SomeQuery", () =>
        HttpResponse.json({ data: { location: { id: "x" } } }),
      ),
    );
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    await fetchGraphQL(null, queryRequest("SomeQuery"), {}, () => {});

    expect(takeRecentFieldErrorMessages()).toEqual([]);
    expect(consoleSpy).not.toHaveBeenCalled();
  });
});

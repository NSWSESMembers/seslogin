import { afterEach, describe, expect, it, vi } from "vitest";
import type { RequestParameters } from "relay-runtime";
import { fetchGraphQL, onGraphQLFieldError } from "./graphql";

vi.mock("./api", () => ({
  getGraphQLEndpoint: () => "https://example.test/graphql",
}));

function request(
  operationKind: "query" | "mutation",
  name = "SomeQuery",
): RequestParameters {
  return {
    id: null,
    cacheID: name,
    metadata: {},
    name,
    operationKind,
    text: `${operationKind} ${name} { __typename }`,
  };
}

function mockResponse(body: unknown) {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue({
      status: 200,
      ok: true,
      json: async () => body,
    }),
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("fetchGraphQL field-error events", () => {
  it("emits a field-error event for a query partial response", async () => {
    mockResponse({
      data: { thing: null },
      errors: [{ message: "boom", path: ["thing", "person"] }],
    });
    const seen: unknown[] = [];
    const off = onGraphQLFieldError((d) => seen.push(d));

    await fetchGraphQL(
      null,
      request("query", "LocationPeriodsQuery"),
      {},
      () => {},
    );
    off();

    expect(seen).toEqual([
      { operationName: "LocationPeriodsQuery", errorCount: 1 },
    ]);
  });

  it("does not emit for a clean query response", async () => {
    mockResponse({ data: { thing: 1 } });
    const seen: unknown[] = [];
    const off = onGraphQLFieldError((d) => seen.push(d));

    await fetchGraphQL(null, request("query"), {}, () => {});
    off();

    expect(seen).toEqual([]);
  });

  it("does not emit for a mutation field error (that path throws instead)", async () => {
    mockResponse({
      data: { doThing: { id: "1" } },
      errors: [{ message: "boom" }],
    });
    const seen: unknown[] = [];
    const off = onGraphQLFieldError((d) => seen.push(d));

    await expect(
      fetchGraphQL(null, request("mutation", "DoThingMutation"), {}, () => {}),
    ).rejects.toThrow();
    off();

    expect(seen).toEqual([]);
  });
});

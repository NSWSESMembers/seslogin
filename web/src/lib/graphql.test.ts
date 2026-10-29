import { afterEach, describe, expect, it, vi } from "vitest";
import type { RequestParameters } from "relay-runtime";
import {
  clockSkewServerTime,
  fetchGraphQL,
  onGraphQLFieldError,
} from "./graphql";

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

describe("fetchGraphQL clock skew", () => {
  const skew = {
    status: 401,
    ok: false,
    json: async () => ({
      errors: [
        { extensions: { code: "CLOCK_SKEW", serverTime: 1_793_000_000 } },
      ],
    }),
  };
  const ok = { status: 200, ok: true, json: async () => ({ data: {} }) };

  it("adopts the server's clock and re-signs once, instead of reporting unauthorized", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValueOnce(skew).mockResolvedValueOnce(ok),
    );
    const sign = vi.fn(async () => "SLKey sig");
    const onUnauthorized = vi.fn();
    const onClockSkew = vi.fn();

    await fetchGraphQL(sign, request("query"), {}, onUnauthorized, onClockSkew);

    expect(onClockSkew).toHaveBeenCalledWith(1_793_000_000);
    expect(sign).toHaveBeenCalledTimes(2);
    expect(onUnauthorized).not.toHaveBeenCalled();
  });

  it("reports unauthorized if the retry is refused too", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(skew));
    const onUnauthorized = vi.fn();

    await expect(
      fetchGraphQL(
        async () => "SLKey sig",
        request("query"),
        {},
        onUnauthorized,
        vi.fn(),
      ),
    ).rejects.toThrow("Unauthorized");
    expect(onUnauthorized).toHaveBeenCalledTimes(1);
  });

  it("treats any other 401 as unauthorized", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        status: 401,
        ok: false,
        json: async () => ({
          errors: [{ message: "Authentication failed" }],
        }),
      }),
    );
    const onUnauthorized = vi.fn();
    const onClockSkew = vi.fn();

    await expect(
      fetchGraphQL(
        async () => "SLKey sig",
        request("query"),
        {},
        onUnauthorized,
        onClockSkew,
      ),
    ).rejects.toThrow("Unauthorized");
    expect(onClockSkew).not.toHaveBeenCalled();
    expect(onUnauthorized).toHaveBeenCalledTimes(1);
  });

  it("reads serverTime only from a CLOCK_SKEW error", () => {
    expect(
      clockSkewServerTime({
        errors: [{ extensions: { code: "CLOCK_SKEW", serverTime: 5 } }],
      }),
    ).toBe(5);
    expect(
      clockSkewServerTime({
        errors: [{ extensions: { code: "OTHER", serverTime: 5 } }],
      }),
    ).toBeNull();
    expect(clockSkewServerTime(null)).toBeNull();
  });
});

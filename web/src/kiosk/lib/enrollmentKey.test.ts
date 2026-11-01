import { afterEach, describe, expect, it, vi } from "vitest";
import { fetchKeySessionId } from "./enrollmentKey";
import { clearClockSkew, getClockSkew, reportClockSkew } from "./clockSkew";
import type { KioskKeyInfo } from "./kioskKey";

vi.mock("../../lib/api", () => ({
  getGraphQLEndpoint: () => "https://example.test/graphql",
}));
vi.mock("./kioskKey", () => ({
  buildSignedAuthHeader: async () => "SLKey abc.1.sig",
}));

const info = { fingerprint: "abc" } as KioskKeyInfo;

function mockFetch(status: number, body: unknown) {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue({
      status,
      ok: status >= 200 && status < 300,
      json: async () => body,
    }),
  );
}

afterEach(() => {
  clearClockSkew();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("fetchKeySessionId", () => {
  it("reports a clock refusal, so the enrollment screen can say so", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    mockFetch(401, {
      errors: [
        { extensions: { code: "CLOCK_SKEW", serverTime: 1_793_000_000 } },
      ],
    });

    expect(await fetchKeySessionId(info)).toBeNull();
    expect(getClockSkew()).not.toBeNull();
  });

  it("clears it on an ordinary 401: the timestamp was checked first, so the clock is fine", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    reportClockSkew(1_793_000_000);
    mockFetch(401, { errors: [{ message: "Authentication failed" }] });

    expect(await fetchKeySessionId(info)).toBeNull();
    expect(getClockSkew()).toBeNull();
  });

  it("clears it once the key resolves to a session", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    reportClockSkew(1_793_000_000);
    mockFetch(200, { data: { session: { id: "s1" } } });

    expect(await fetchKeySessionId(info)).toBe("s1");
    expect(getClockSkew()).toBeNull();
  });

  it("leaves it alone when the server is unavailable", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    reportClockSkew(1_793_000_000);
    mockFetch(503, null);

    expect(await fetchKeySessionId(info)).toBeNull();
    expect(getClockSkew()).not.toBeNull();
  });
});

import { afterEach, describe, expect, it, vi } from "vitest";
import {
  CLIENT_INFO_HEADER,
  clientHeaders,
  resetClientInfo,
  setContactFailureSource,
  setKioskProfile,
  type ClientInfo,
} from "./clientInfo";
import { CLIENT_VERSION_HEADER } from "./clientVersion";

function parse(): ClientInfo {
  const headers = clientHeaders();
  return JSON.parse(headers[CLIENT_INFO_HEADER]) as ClientInfo;
}

describe("clientHeaders", () => {
  afterEach(() => {
    resetClientInfo();
    vi.restoreAllMocks();
  });

  it("always sends the version header alongside the info header", () => {
    const headers = clientHeaders();
    expect(headers[CLIENT_VERSION_HEADER]).toBeTypeOf("string");
    expect(headers[CLIENT_INFO_HEADER]).toBeTypeOf("string");
  });

  it("reports the facts it can observe without any registration", () => {
    const info = parse();
    expect(info.env).toBe("dev");
    expect(info.origin).toBe(window.location.origin);
    expect(info.apiUrl).toBeTypeOf("string");
    expect(info.timezone).toBeTypeOf("string");
    expect(info.clockMs).toBeGreaterThan(0);
  });

  it("reports the kiosk profile once the kiosk registers it", () => {
    expect(parse().profile).toBeUndefined();
    setKioskProfile("north-shed");
    expect(parse().profile).toBe("north-shed");
  });

  it("reports the contact failure count from the registered source", () => {
    setContactFailureSource(() => 7);
    expect(parse().contactFailures).toBe(7);
  });

  /**
   * The server treats a present field as a claim and an absent one as "not reported",
   * so sending an explicit null would assert something the client doesn't actually know.
   */
  it("omits unknown fields rather than sending nulls", () => {
    const info = parse();
    expect(info).not.toHaveProperty("profile");
    expect(info).not.toHaveProperty("pendingVersion");
    expect(Object.values(info).every((value) => value != null)).toBe(true);
  });

  /**
   * The whole point of the try/catch: a kiosk must never fail to sign someone in
   * because a diagnostic field couldn't be collected.
   */
  it("still sends the version header when collection throws", () => {
    vi.spyOn(Intl, "DateTimeFormat").mockImplementation(() => {
      throw new Error("no Intl here");
    });
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});

    const headers = clientHeaders();
    expect(headers[CLIENT_VERSION_HEADER]).toBeTypeOf("string");
    expect(headers[CLIENT_INFO_HEADER]).toBeUndefined();
    expect(warn).toHaveBeenCalled();
  });
});

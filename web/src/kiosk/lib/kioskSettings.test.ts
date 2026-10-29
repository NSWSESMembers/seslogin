import { beforeEach, describe, expect, it } from "vitest";
import { markKeyEnrolled, readAppSettings } from "./kioskSettings";

beforeEach(() => {
  localStorage.clear();
});

describe("kioskSettings", () => {
  it("drops a session JWT left behind by the removed setup-code enrolment", () => {
    localStorage.setItem(
      "kiosk_default",
      JSON.stringify({
        scanAuthToken: "eyJ...",
        scanAuthTokenIssuedAt: 1,
        authMode: "jwt",
        other: "kept",
      }),
    );

    expect(readAppSettings("default")).toEqual({
      authMode: "jwt",
      other: "kept",
    });
    expect(JSON.parse(localStorage.getItem("kiosk_default")!)).toEqual({
      authMode: "jwt",
      other: "kept",
    });
  });

  it("records key mode, keeping the other settings", () => {
    localStorage.setItem("kiosk_lobby", JSON.stringify({ other: "kept" }));
    markKeyEnrolled("lobby");
    expect(readAppSettings("lobby")).toEqual({
      authMode: "key",
      other: "kept",
    });
  });
});

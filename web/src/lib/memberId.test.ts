import { describe, it, expect } from "vitest";
import { isValidMemberIdText } from "./memberId";

describe("isValidMemberIdText", () => {
  it("identifies valid member Ids correctly", () => {
    expect(isValidMemberIdText("40050107")).toEqual(true);
    expect(isValidMemberIdText("40000001")).toEqual(true);
  });

  it("does not exclude member Ids that do not start with 400", () => {
    expect(isValidMemberIdText("00034567")).toEqual(true);
  });


  it("rejects member Ids that are too short", () => {
    expect(isValidMemberIdText("4001234")).toEqual(false);
  });

  it("rejects member Ids that are too long", () => {
    expect(isValidMemberIdText("123456789")).toEqual(false);
  });


  it("rejects member Ids that contain non-numeric characters", () => {
    expect(isValidMemberIdText("4001234A")).toEqual(false);
    expect(isValidMemberIdText("40012 45")).toEqual(false);
    expect(isValidMemberIdText("4001.345")).toEqual(false);
    expect(isValidMemberIdText("400:2345")).toEqual(false);

    expect(isValidMemberIdText("Some attempt at an injection attack")).toEqual(
      false,
    );
  });
});

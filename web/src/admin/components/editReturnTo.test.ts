import { describe, expect, it } from "vitest";
import { editReturnState, editReturnTo } from "./editReturnTo";

describe("editReturnTo", () => {
  it("returns to the list the edit was opened from", () => {
    expect(
      editReturnTo(
        editReturnState("/admin/activity/current"),
        "/admin/activity",
      ),
    ).toBe("/admin/activity/current");
  });

  it("keeps the list's query string", () => {
    expect(
      editReturnTo(
        editReturnState("/admin/members/activity/m1?from=2026-01-01"),
        "/admin/activity",
      ),
    ).toBe("/admin/members/activity/m1?from=2026-01-01");
  });

  it("falls back when the form was opened directly", () => {
    expect(editReturnTo(null, "/admin/activity")).toBe("/admin/activity");
    expect(editReturnTo({}, "/admin/activity")).toBe("/admin/activity");
  });

  it("refuses anything outside the admin site", () => {
    for (const returnTo of [
      "https://evil.example/",
      "//evil.example/",
      "/period",
      42,
    ]) {
      expect(editReturnTo({ returnTo }, "/admin/activity")).toBe(
        "/admin/activity",
      );
    }
  });
});

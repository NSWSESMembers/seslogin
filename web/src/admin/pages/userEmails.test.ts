import { describe, it, expect } from "vitest";
import { parseEmails } from "./userEmails";

describe("parseEmails", () => {
  it("accepts a single address", () => {
    expect(parseEmails("  a@example.com ")).toEqual({
      emails: ["a@example.com"],
      invalid: [],
    });
  });

  it("splits on commas, semicolons, spaces and new lines", () => {
    expect(
      parseEmails(
        "a@example.com, b@example.com;c@example.com\nd@example.com e@example.com",
      ),
    ).toEqual({
      emails: [
        "a@example.com",
        "b@example.com",
        "c@example.com",
        "d@example.com",
        "e@example.com",
      ],
      invalid: [],
    });
  });

  it("drops duplicates case-insensitively, keeping the first spelling", () => {
    expect(parseEmails("Bob@Example.com\nbob@example.com").emails).toEqual([
      "Bob@Example.com",
    ]);
  });

  it("strips angle brackets from a pasted address", () => {
    expect(parseEmails("<a@example.com>").emails).toEqual(["a@example.com"]);
  });

  it("reports entries that aren't email addresses", () => {
    expect(parseEmails("a@example.com, nope, b@example")).toEqual({
      emails: ["a@example.com"],
      invalid: ["nope", "b@example"],
    });
  });

  it("returns nothing for blank input", () => {
    expect(parseEmails(" \n , ")).toEqual({ emails: [], invalid: [] });
  });
});

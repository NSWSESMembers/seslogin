import { describe, expect, it } from "vitest";
import { shortenGitRev } from "./clientVersion";

describe("shortenGitRev", () => {
  it("shortens a full 40-character SHA to 7 characters", () => {
    expect(shortenGitRev("a1b2c3d4e5f6789012345678901234567890abcd")).toBe(
      "a1b2c3d",
    );
  });

  it("leaves a value that isn't a full SHA unchanged", () => {
    expect(shortenGitRev("dev")).toBe("dev");
    expect(shortenGitRev("abc1234")).toBe("abc1234");
  });

  it("trims surrounding whitespace before checking length", () => {
    expect(shortenGitRev("  a1b2c3d4e5f6789012345678901234567890abcd  ")).toBe(
      "a1b2c3d",
    );
  });
});

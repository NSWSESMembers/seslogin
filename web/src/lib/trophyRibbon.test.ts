import { describe, it, expect } from "vitest";
import { ribbonLines } from "./trophyRibbon";

describe("ribbonLines", () => {
  it("drops the closing Unit and keeps a short name on one line", () => {
    expect(ribbonLines("Test A Unit")).toEqual({
      lines: ["TEST A"],
      fontSize: 9.5,
    });
  });

  it("shrinks a longer name to fit between the ribbon's ends", () => {
    const { lines, fontSize } = ribbonLines("Sutherland Shire Unit");
    expect(lines).toEqual(["SUTHERLAND SHIRE"]);
    expect(fontSize * 0.74 * lines[0].length).toBeLessThanOrEqual(96);
    expect(fontSize).toBeLessThan(9.5);
  });

  it("splits a name too long for one readable line near its middle", () => {
    const { lines, fontSize } = ribbonLines(
      "Upper Lachlan Shire and Crookwell Region Unit",
    );
    expect(lines).toEqual(["UPPER LACHLAN SHIRE", "AND CROOKWELL REGION"]);
    expect(fontSize * 0.74 * 20).toBeLessThanOrEqual(96);
  });

  it("splits a name too long for one line at a space", () => {
    expect(ribbonLines("Port Macquarie Hastings Unit").lines).toEqual([
      "PORT MACQUARIE",
      "HASTINGS",
    ]);
  });

  it("keeps a name that is only Unit", () => {
    expect(ribbonLines("Unit").lines).toEqual(["UNIT"]);
  });
});

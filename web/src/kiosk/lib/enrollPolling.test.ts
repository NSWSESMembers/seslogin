import { describe, it, expect } from "vitest";
import { pollDelayMs } from "./enrollPolling";

describe("pollDelayMs", () => {
  it("polls every 5s for the first 2 minutes", () => {
    expect(pollDelayMs(0)).toBe(5000);
    expect(pollDelayMs(2 * 60 * 1000 - 1)).toBe(5000);
  });

  it("polls every minute between 2 minutes and 1 hour", () => {
    expect(pollDelayMs(2 * 60 * 1000)).toBe(60 * 1000);
    expect(pollDelayMs(60 * 60 * 1000 - 1)).toBe(60 * 1000);
  });

  it("polls every 5 minutes after 1 hour", () => {
    expect(pollDelayMs(60 * 60 * 1000)).toBe(5 * 60 * 1000);
    expect(pollDelayMs(10 * 60 * 60 * 1000)).toBe(5 * 60 * 1000);
  });
});

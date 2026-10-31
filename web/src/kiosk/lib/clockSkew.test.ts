import { afterEach, describe, expect, it, vi } from "vitest";
import {
  clearClockSkew,
  describeClockOffset,
  getClockSkew,
  reportClockSkew,
} from "./clockSkew";

afterEach(() => {
  clearClockSkew();
  vi.useRealTimers();
});

describe("clock skew state", () => {
  it("records the server's clock relative to the device's until cleared", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(1_793_000_600_000));
    vi.spyOn(console, "warn").mockImplementation(() => {});

    reportClockSkew(1_793_000_000);
    expect(getClockSkew()).toEqual({ offsetMs: -600_000 });

    clearClockSkew();
    expect(getClockSkew()).toBeNull();
  });
});

describe("describeClockOffset", () => {
  it.each([
    [-600_000, "10 minutes fast"],
    [600_000, "10 minutes slow"],
    [-60_000, "1 minute fast"],
    [-330_000, "6 minutes fast"],
    [3 * 3600_000, "3 hours slow"],
    [-400 * 86_400_000, "400 days fast"],
    [-45_000, "45 seconds fast"],
  ])("%d ms → %s", (offsetMs, words) => {
    expect(describeClockOffset(offsetMs)).toBe(words);
  });
});

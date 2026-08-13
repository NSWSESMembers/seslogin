import { describe, expect, it } from "vitest";
import { formatShortDuration } from "./time";

describe("formatShortDuration", () => {
  it("shows seconds under a minute", () => {
    expect(formatShortDuration(0)).toBe("0s");
    expect(formatShortDuration(42.7)).toBe("42s");
    expect(formatShortDuration(59)).toBe("59s");
  });

  it("shows minutes and seconds under an hour", () => {
    expect(formatShortDuration(60)).toBe("1m 00s");
    expect(formatShortDuration(185)).toBe("3m 05s");
    expect(formatShortDuration(3599)).toBe("59m 59s");
  });

  it("shows hours and minutes under a day", () => {
    expect(formatShortDuration(3600)).toBe("1h 00m");
    expect(formatShortDuration(8040)).toBe("2h 14m");
  });

  it("shows days and hours beyond a day", () => {
    expect(formatShortDuration(86400)).toBe("1d 0h");
    expect(formatShortDuration(273600)).toBe("3d 4h");
  });

  it("clamps negative durations to zero", () => {
    expect(formatShortDuration(-5)).toBe("0s");
  });
});

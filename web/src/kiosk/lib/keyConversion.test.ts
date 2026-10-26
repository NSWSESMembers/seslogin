import { describe, expect, it } from "vitest";
import {
  TEST_UPTIME_MS,
  conversionEligibleAt,
  isEligibleForKeyConversion,
} from "./keyConversion";

const sydney = new Intl.DateTimeFormat("en-AU", {
  timeZone: "Australia/Sydney",
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
});

function sydneyTime(ms: number) {
  return sydney.format(new Date(ms));
}

const prod = { environment: "prod", uptimeMs: 0 };

describe("conversionEligibleAt", () => {
  it("pins the first two kiosks to Saturday morning, Sydney time", () => {
    expect(sydneyTime(conversionEligibleAt("NjHCubqnmslo"))).toBe(
      "17/10/2026, 09:00",
    );
    expect(sydneyTime(conversionEligibleAt("UL23XGaM9wD1"))).toBe(
      "17/10/2026, 10:00",
    );
  });

  it("spreads every other kiosk across 20:00–21:00 Saturday, deterministically", () => {
    const start = Date.parse("2026-10-17T09:00:00Z");
    const end = Date.parse("2026-10-17T10:00:00Z");
    const ids = Array.from({ length: 200 }, (_, i) => `Sess${i}abcdef`);
    const times = ids.map(conversionEligibleAt);
    for (const t of times) {
      expect(t).toBeGreaterThanOrEqual(start);
      expect(t).toBeLessThan(end);
      expect(sydneyTime(t)).toMatch(/^17\/10\/2026, 20:/);
    }
    expect(ids.map(conversionEligibleAt)).toEqual(times);
    // Spread, not bunched: each quarter of the hour gets some kiosks.
    const quarters = new Set(
      times.map((t) => Math.floor((t - start) / (15 * 60 * 1000))),
    );
    expect(quarters).toEqual(new Set([0, 1, 2, 3]));
  });
});

describe("isEligibleForKeyConversion", () => {
  it("turns on at the session's time and not before", () => {
    const at = conversionEligibleAt("NjHCubqnmslo");
    expect(
      isEligibleForKeyConversion("NjHCubqnmslo", { ...prod, now: at - 1 }),
    ).toBe(false);
    expect(
      isEligibleForKeyConversion("NjHCubqnmslo", { ...prod, now: at }),
    ).toBe(true);
    // The other pinned kiosk is still an hour away.
    expect(
      isEligibleForKeyConversion("UL23XGaM9wD1", { ...prod, now: at }),
    ).toBe(false);
  });

  it("lets any kiosk convert after two minutes up on dev and test", () => {
    const now = Date.parse("2026-01-01T00:00:00Z");
    for (const environment of ["dev", "test"]) {
      expect(
        isEligibleForKeyConversion("AnyKiosk", {
          now,
          environment,
          uptimeMs: TEST_UPTIME_MS - 1,
        }),
      ).toBe(false);
      expect(
        isEligibleForKeyConversion("AnyKiosk", {
          now,
          environment,
          uptimeMs: TEST_UPTIME_MS,
        }),
      ).toBe(true);
    }
  });

  it("never applies the uptime shortcut on prod, preprod or qa", () => {
    const now = Date.parse("2026-01-01T00:00:00Z");
    for (const environment of ["prod", "preprod", "qa"]) {
      expect(
        isEligibleForKeyConversion("AnyKiosk", {
          now,
          environment,
          uptimeMs: 24 * 60 * 60 * 1000,
        }),
      ).toBe(false);
    }
  });
});

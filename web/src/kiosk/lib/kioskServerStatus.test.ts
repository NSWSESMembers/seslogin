import { beforeEach, describe, expect, it } from "vitest";
import {
  getKioskServerStatus,
  recordServerContactFailure,
  recordServerContactSuccess,
  resetKioskServerStatus,
} from "./kioskServerStatus";

describe("kioskServerStatus", () => {
  beforeEach(() => {
    resetKioskServerStatus();
  });

  it("starts with nothing recorded", () => {
    expect(getKioskServerStatus()).toEqual({
      lastSuccessAt: null,
      lastAttemptAt: null,
      lastFailureAt: null,
      lastErrorMessage: null,
      keyExpiresAt: null,
      failureCount: 0,
    });
  });

  it("records a success with the key expiry", () => {
    const before = Date.now();
    recordServerContactSuccess(1234);
    const status = getKioskServerStatus();

    expect(status.lastSuccessAt).toBeGreaterThanOrEqual(before);
    expect(status.lastAttemptAt).toBe(status.lastSuccessAt);
    expect(status.keyExpiresAt).toBe(1234);
  });

  it("records a failure without clobbering the last success", () => {
    recordServerContactSuccess(null);
    const successAt = getKioskServerStatus().lastSuccessAt;

    recordServerContactFailure(new Error("network down"));
    const status = getKioskServerStatus();

    expect(status.lastSuccessAt).toBe(successAt);
    expect(status.lastFailureAt).not.toBeNull();
    expect(status.lastErrorMessage).toBe("network down");
  });

  it("clears a previous failure on the next success", () => {
    recordServerContactFailure(new Error("network down"));
    recordServerContactSuccess(null);
    const status = getKioskServerStatus();

    expect(status.lastFailureAt).toBeNull();
    expect(status.lastErrorMessage).toBeNull();
  });

  it("describes non-Error failures", () => {
    recordServerContactFailure("plain string");
    expect(getKioskServerStatus().lastErrorMessage).toBe("plain string");

    recordServerContactFailure({ status: 502 });
    expect(getKioskServerStatus().lastErrorMessage).toBe('{"status":502}');
  });

  // The count is what makes a kiosk that drops off and recovers all day visible: every
  // other field describes only the present moment, in which it looks fine.
  it("counts failures cumulatively and does not reset them on success", () => {
    recordServerContactFailure(new Error("boom"));
    recordServerContactFailure(new Error("boom again"));
    expect(getKioskServerStatus().failureCount).toBe(2);

    recordServerContactSuccess(1234);
    expect(getKioskServerStatus().failureCount).toBe(2);
    expect(getKioskServerStatus().lastFailureAt).toBeNull();

    recordServerContactFailure(new Error("and again"));
    expect(getKioskServerStatus().failureCount).toBe(3);
  });
});

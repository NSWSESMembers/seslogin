// @vitest-environment jsdom

import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi, type Mock } from "vitest";

vi.mock("./kioskKey", () => ({
  getOrCreateKioskKey: vi.fn(),
}));

vi.mock("qrcode", () => ({
  default: { toDataURL: vi.fn() },
}));

vi.mock("./enrollmentKey", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./enrollmentKey")>()),
  submitEnrollmentKey: vi.fn(),
}));

import QRCode from "qrcode";
import { getOrCreateKioskKey } from "./kioskKey";
import { submitEnrollmentKey } from "./enrollmentKey";
import { useEnrollmentQr } from "./useEnrollmentQr";

const keyInfo = {
  keyPair: {} as CryptoKeyPair,
  publicKeySpkiB64: "cHVia2V5",
  fingerprint: "abc123",
};

// `toDataURL` is heavily overloaded, so vi.mocked() resolves it to the void-returning
// callback form; the mock only ever stands in for the promise form.
const toDataURL = QRCode.toDataURL as unknown as Mock<
  (text: string) => Promise<string>
>;

function mockKeyAndQr() {
  vi.mocked(getOrCreateKioskKey).mockResolvedValue(keyInfo);
  toDataURL.mockResolvedValue("data:image/png;base64,QR");
}

describe("useEnrollmentQr", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("does nothing at all until enabled", async () => {
    mockKeyAndQr();
    const { result } = renderHook(() => useEnrollmentQr("default", false));

    expect(getOrCreateKioskKey).not.toHaveBeenCalled();
    expect(submitEnrollmentKey).not.toHaveBeenCalled();
    expect(result.current.qrDataUrl).toBeNull();
  });

  it("publishes the key and renders a QR code pointing at the enroll page", async () => {
    mockKeyAndQr();
    vi.mocked(submitEnrollmentKey).mockResolvedValue();
    const { result } = renderHook(() => useEnrollmentQr("default"));

    await waitFor(() => expect(result.current.qrDataUrl).not.toBeNull());
    expect(result.current.fingerprint).toBe("abc123");
    expect(result.current.enrollUrl).toBe(
      `${window.location.origin}/admin/sessions/enroll?fp=abc123`,
    );
    expect(toDataURL.mock.calls[0][0]).toBe(result.current.enrollUrl);
    await waitFor(() =>
      expect(submitEnrollmentKey).toHaveBeenCalledWith(keyInfo),
    );
    expect(result.current.error).toBeNull();
  });

  it("reports a failure to publish the key, since the code won't work without it", async () => {
    mockKeyAndQr();
    vi.mocked(submitEnrollmentKey).mockRejectedValue(
      new Error("Failed to fetch"),
    );
    vi.spyOn(console, "error").mockImplementation(() => {});
    const { result } = renderHook(() => useEnrollmentQr("default"));

    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.error).toContain("Failed to fetch");
  });

  it("clears its state when disabled again", async () => {
    mockKeyAndQr();
    vi.mocked(submitEnrollmentKey).mockResolvedValue();
    const { result, rerender } = renderHook(
      ({ enabled }: { enabled: boolean }) =>
        useEnrollmentQr("default", enabled),
      { initialProps: { enabled: true } },
    );

    await waitFor(() => expect(result.current.qrDataUrl).not.toBeNull());
    rerender({ enabled: false });

    expect(result.current.qrDataUrl).toBeNull();
    expect(result.current.fingerprint).toBeNull();
  });
});

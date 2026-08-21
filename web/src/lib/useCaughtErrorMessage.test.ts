import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useCaughtErrorMessage } from "./useCaughtErrorMessage";
import { relayFieldLogger } from "./relayFieldLogger";

function bufferMessage(message: string) {
  relayFieldLogger({
    kind: "relay_field_payload.error" as const,
    owner: "SomeQuery",
    fieldPath: "location.periodSummaryByCategory.0.category",
    error: { message, path: [], severity: "ERROR" as const },
    shouldThrow: true,
    handled: false,
  });
}

describe("useCaughtErrorMessage", () => {
  beforeEach(() => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    vi.spyOn(console, "warn").mockImplementation(() => {});
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("starts with no message", () => {
    const { result } = renderHook(() => useCaughtErrorMessage());
    expect(result.current.message).toBeNull();
  });

  it("resolves the real message via onError", () => {
    bufferMessage("Category with ID abc123 missing");
    const { result } = renderHook(() => useCaughtErrorMessage());

    act(() => {
      result.current.onError(
        new Error("Relay: Missing expected data at path 'x' in 'y'."),
      );
    });

    expect(result.current.message).toBe("Category with ID abc123 missing");
  });

  it("clears the message on reset", () => {
    bufferMessage("some message");
    const { result } = renderHook(() => useCaughtErrorMessage());

    act(() => {
      result.current.onError(new Error("Relay: something"));
    });
    expect(result.current.message).not.toBeNull();

    act(() => {
      result.current.reset();
    });
    expect(result.current.message).toBeNull();
  });
});

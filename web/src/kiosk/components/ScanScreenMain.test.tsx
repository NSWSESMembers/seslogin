import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vitest } from "vitest";
import ScanScreenMain from "./ScanScreenMain";
import { resumeScanFocus, suspendScanFocus } from "../lib/scanFocusLeases";

const LEASE_ID = "test:overlay";
const REFOCUS_DELAY_MS = 2_000;

function renderScanScreen(): HTMLElement {
  render(
    <ScanScreenMain
      screenPosition="center"
      submitDisabled={false}
      transactionState={{ transactions: [] }}
      onSubmit={async () => {}}
      validateMemberId={() => true}
    />,
  );
  return screen.getByRole("textbox");
}

// Stands in for whatever the operator is actually typing into (a dialog field).
// Appended outside the RTL container, so it needs cleaning up by hand.
let otherInput: HTMLInputElement | null = null;

function focusSomethingElse(): HTMLInputElement {
  const other = document.createElement("input");
  document.body.appendChild(other);
  otherInput = other;
  act(() => {
    other.focus();
  });
  return other;
}

describe("ScanScreenMain", () => {
  beforeEach(() => {
    vitest.useFakeTimers();
  });

  afterEach(() => {
    resumeScanFocus(LEASE_ID);
    otherInput?.remove();
    otherInput = null;
    vitest.useRealTimers();
  });

  it("takes focus back after the input loses it", () => {
    const input = renderScanScreen();
    expect(document.activeElement).toBe(input);

    const other = focusSomethingElse();
    expect(document.activeElement).toBe(other);

    act(() => {
      vitest.advanceTimersByTime(REFOCUS_DELAY_MS);
    });
    expect(document.activeElement).toBe(input);
  });

  it("does not steal focus while a scan focus lease is held", () => {
    renderScanScreen();
    const other = focusSomethingElse();

    act(() => {
      suspendScanFocus(LEASE_ID);
    });

    act(() => {
      vitest.advanceTimersByTime(REFOCUS_DELAY_MS * 5);
    });
    expect(document.activeElement).toBe(other);
  });

  it("takes focus back once the last lease is released", () => {
    const input = renderScanScreen();
    focusSomethingElse();

    act(() => {
      suspendScanFocus(LEASE_ID);
    });
    act(() => {
      vitest.advanceTimersByTime(REFOCUS_DELAY_MS);
    });

    act(() => {
      resumeScanFocus(LEASE_ID);
    });
    expect(document.activeElement).toBe(input);
  });
});

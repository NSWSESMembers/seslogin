import { render, screen } from "@testing-library/react";
import UserEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vitest } from "vitest";
import KioskStatusDialog from "./KioskStatusDialog";
import { KioskSessionContext } from "./KioskSessionContext";
import { KioskEnvironmentContext } from "./KioskEnvironmentContext";
import {
  recordServerContactFailure,
  recordServerContactSuccess,
  resetKioskServerStatus,
} from "../lib/kioskServerStatus";
import { setEnvironmentInfo } from "../../lib/environmentInfo";

const session = {
  id: "sess123",
  name: "Front Desk",
  config: { guests: true, theme: "dark", smallCategories: false },
  location: { id: "loc456", name: "Test Unit" },
};

function renderDialog(onClose = vitest.fn()) {
  return render(
    <KioskEnvironmentContext.Provider
      value={{ setToken: vitest.fn(), profile: "default", authMode: "key" }}
    >
      <KioskSessionContext.Provider value={{ session }}>
        <KioskStatusDialog onClose={onClose} />
      </KioskSessionContext.Provider>
    </KioskEnvironmentContext.Provider>,
  );
}

describe("KioskStatusDialog", () => {
  beforeEach(() => {
    resetKioskServerStatus();
  });

  afterEach(() => {
    vitest.restoreAllMocks();
  });

  it("shows the kiosk, location and check-in details", () => {
    recordServerContactSuccess(null);
    const { container } = renderDialog();

    expect(screen.getByText("Front Desk")).toBeDefined();
    expect(screen.getByText("sess123")).toBeDefined();
    expect(screen.getByText("Test Unit")).toBeDefined();
    expect(screen.getByText("loc456")).toBeDefined();
    expect(
      container.textContent?.includes("Last server check-in"),
    ).toBeTruthy();
    expect(screen.getByText(/0s ago/)).toBeDefined();
    expect(screen.getByText("enrolled key")).toBeDefined();
  });

  it("reports when the kiosk has never reached the server", () => {
    renderDialog();
    expect(screen.getByText("never")).toBeDefined();
  });

  it("shows the last failure alongside the last success", () => {
    recordServerContactSuccess(null);
    recordServerContactFailure(new Error("Failed to fetch"));
    const { container } = renderDialog();

    expect(container.textContent?.includes("Last failure")).toBeTruthy();
    expect(screen.getByText(/Failed to fetch/)).toBeDefined();
  });

  it("shows what the API server reported about itself", () => {
    setEnvironmentInfo({ gitRev: "abc1234", isProdDb: true });
    renderDialog();

    expect(screen.getByText("abc1234")).toBeDefined();
    expect(screen.getByText("production")).toBeDefined();
  });

  it("calls out a non-production database", () => {
    setEnvironmentInfo({ gitRev: "abc1234", isProdDb: false });
    renderDialog();

    expect(screen.getByText("NOT production")).toBeDefined();
  });

  it("lists only the enabled config flags", () => {
    renderDialog();
    expect(screen.getByText('guests, theme="dark"')).toBeDefined();
  });

  it("contains no links that could navigate the kiosk away", () => {
    recordServerContactSuccess(null);
    const { container } = renderDialog();
    expect(container.querySelectorAll("a")).toHaveLength(0);
  });

  it("closes when Close is tapped", async () => {
    const onClose = vitest.fn();
    const user = UserEvent.setup();
    renderDialog(onClose);

    await user.click(screen.getByText("Close"));
    expect(onClose).toHaveBeenCalled();
  });

  it("reloads the page when Reload is tapped", async () => {
    const reload = vitest.fn();
    const originalLocation = window.location;
    Object.defineProperty(window, "location", {
      configurable: true,
      value: { ...originalLocation, reload },
    });

    try {
      const user = UserEvent.setup();
      renderDialog();
      await user.click(screen.getByText("Reload"));
      expect(reload).toHaveBeenCalledTimes(1);
    } finally {
      Object.defineProperty(window, "location", {
        configurable: true,
        value: originalLocation,
      });
    }
  });
});

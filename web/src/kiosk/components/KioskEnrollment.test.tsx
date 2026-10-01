import "@testing-library/jest-dom/vitest";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import UserEvent from "@testing-library/user-event";

vi.mock("../lib/useEnrollmentQr", () => ({
  useEnrollmentQr: () => ({
    info: null,
    fingerprint: "abc123",
    enrollUrl: "https://example.test/enroll?fp=abc123",
    qrDataUrl: "data:image/png;base64,QR",
  }),
}));

import KioskEnrollment from "./KioskEnrollment";

// iPadOS asks for desktop sites by default, so an iPad and a Mac send the same user
// agent; only the touch points tell them apart.
const SAFARI_UA =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15";

function setDevice({
  ua,
  touchPoints,
  standalone = false,
}: {
  ua: string;
  touchPoints: number;
  standalone?: boolean;
}) {
  vi.spyOn(navigator, "userAgent", "get").mockReturnValue(ua);
  Object.defineProperty(navigator, "maxTouchPoints", {
    configurable: true,
    get: () => touchPoints,
  });
  Object.defineProperty(navigator, "standalone", {
    configurable: true,
    get: () => standalone,
  });
}

function renderEnrollment() {
  return render(
    <KioskEnrollment
      profile="default"
      onEnrolled={() => {}}
      onUseCodeInstead={() => {}}
    />,
  );
}

describe("KioskEnrollment on an iPad", () => {
  beforeEach(() => {
    sessionStorage.clear();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    Reflect.deleteProperty(navigator, "maxTouchPoints");
    Reflect.deleteProperty(navigator, "standalone");
  });

  it("offers the Home Screen instead of the QR code in a browser tab", () => {
    setDevice({ ua: SAFARI_UA, touchPoints: 5 });
    renderEnrollment();
    expect(
      screen.getByText("Add this kiosk to the Home Screen"),
    ).toBeInTheDocument();
    expect(
      screen.queryByAltText("Kiosk enrollment QR code"),
    ).not.toBeInTheDocument();
  });

  it("shows the QR code after Ignore, and remembers it for the tab", async () => {
    setDevice({ ua: SAFARI_UA, touchPoints: 5 });
    const { unmount } = renderEnrollment();
    await UserEvent.setup().click(
      screen.getByRole("button", { name: "Ignore" }),
    );
    expect(screen.getByAltText("Kiosk enrollment QR code")).toBeInTheDocument();

    unmount();
    renderEnrollment();
    expect(screen.getByAltText("Kiosk enrollment QR code")).toBeInTheDocument();
  });

  it("goes straight to the QR code when launched from the Home Screen", () => {
    setDevice({ ua: SAFARI_UA, touchPoints: 5, standalone: true });
    renderEnrollment();
    expect(screen.getByAltText("Kiosk enrollment QR code")).toBeInTheDocument();
  });

  it("goes straight to the QR code on a Mac, which has no touch screen", () => {
    setDevice({ ua: SAFARI_UA, touchPoints: 0 });
    renderEnrollment();
    expect(screen.getByAltText("Kiosk enrollment QR code")).toBeInTheDocument();
  });
});

import "@testing-library/jest-dom/vitest";
import { vi, describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import KioskMain from "./KioskMain";
import type { KioskSession } from "./components/KioskSessionContext";
import * as KioskTokenSessionFetcher from "./components/KioskTokenSessionFetcher";

describe("KioskMain", () => {
  it("renders the main screen given a valid session", () => {
    const settings = {
      scanAuthToken: "fun-token",
      scanAuthTokenIssuedAt: new Date().getTime(),
    };

    const mockSession: KioskSession = {
      id: "mockId",
      name: "mockName",
      config: {},
      location: {
        id: "mockLocationId",
        name: "mockLocation",
      },
    };

    const getItemSpy = vi.spyOn(localStorage, "getItem");
    getItemSpy.mockReturnValue(JSON.stringify(settings));

    const startKioskTokenSessionFetcherSpy = vi.spyOn(
      KioskTokenSessionFetcher,
      "default",
    );
    startKioskTokenSessionFetcherSpy.mockImplementation(
      ({
        setSession,
      }: {
        setSession: (session: KioskSession | null) => void;
      }) => {
        setSession(mockSession);
        return () => {};
      },
    );

    render(<KioskMain />);
    expect(getItemSpy).toHaveBeenCalledOnce();
    expect(startKioskTokenSessionFetcherSpy).toHaveBeenCalledOnce();
    expect(
      screen.getByText("Please enter or scan your SES ID"),
    ).toBeInTheDocument();
  });
});

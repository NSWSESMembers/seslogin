// @vitest-environment jsdom

import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import useSelectedLocation from "./useSelectedLocation";

vi.mock("../../lib/settings", () => ({
  useSettings: vi.fn(),
}));

vi.mock("./useUserInfo", () => ({
  useUserInfo: vi.fn(),
}));

import { useSettings } from "../../lib/settings";
import { useUserInfo } from "./useUserInfo";

describe("useSelectedLocation", () => {
  it("throws when no location ID is selected", () => {
    vi.mocked(useSettings).mockReturnValue({
      locationId: null,
    });
    vi.mocked(useUserInfo).mockReturnValue({
      id: "user-1",
      email: "tester@example.com",
      isSuper: false,
      isDev: false,
      locations: [
        {
          id: "loc-1",
          name: "HQ",
          enabled: true,
          gamificationEnabled: true,
        },
      ],
    });

    expect(() => renderHook(() => useSelectedLocation())).toThrow(
      "No location selected",
    );
  });

  it("throws when selected location ID is not in user locations", () => {
    vi.mocked(useSettings).mockReturnValue({
      locationId: "loc-missing",
    });
    vi.mocked(useUserInfo).mockReturnValue({
      id: "user-1",
      email: "tester@example.com",
      isSuper: false,
      isDev: false,
      locations: [
        {
          id: "loc-1",
          name: "HQ",
          enabled: true,
          gamificationEnabled: true,
        },
      ],
    });

    expect(() => renderHook(() => useSelectedLocation())).toThrow(
      "Selected location is not available",
    );
  });

  it("returns matching location when selected location ID exists", () => {
    vi.mocked(useSettings).mockReturnValue({
      locationId: "loc-2",
    });
    vi.mocked(useUserInfo).mockReturnValue({
      id: "user-1",
      email: "tester@example.com",
      isSuper: false,
      isDev: false,
      locations: [
        {
          id: "loc-1",
          name: "HQ",
          enabled: true,
          gamificationEnabled: true,
        },
        {
          id: "loc-2",
          name: "Downtown",
          enabled: true,
          gamificationEnabled: false,
        },
      ],
    });

    const { result } = renderHook(() => useSelectedLocation());

    expect(result.current).toEqual({
      id: "loc-2",
      name: "Downtown",
      enabled: true,
      gamificationEnabled: false,
    });
  });
});

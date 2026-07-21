import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { useSettings, useSettingsDispatch } from "../../lib/settings";
import { useUserInfo } from "./useUserInfo";
import { getLocationById, getSelectedLocationId } from "./useSelectedLocation";
import TextInput from "../../components/ui/TextInput";
import { OptionList, OptionButtonRow } from "../../components/ui/OptionList";

interface LocationSelectorProps {
  children?: ReactNode;
}

export default function LocationSelector({ children }: LocationSelectorProps) {
  const settings = useSettings();
  const settingsDispatch = useSettingsDispatch();
  const { locations } = useUserInfo();
  const [filter, setFilter] = useState("");
  const enabledLocations = locations
    .filter((loc) => loc.enabled)
    .sort((a, b) => a.name.localeCompare(b.name));
  const filteredLocations = enabledLocations.filter((loc) =>
    loc.name.toLowerCase().includes(filter.trim().toLowerCase()),
  );
  const selectedLocationId = getSelectedLocationId(settings);
  const selectedLocation =
    selectedLocationId == null
      ? null
      : getLocationById(locations, selectedLocationId);

  useEffect(() => {
    if (selectedLocation != null) {
      return;
    }

    if (enabledLocations.length === 1) {
      const only = enabledLocations[0];
      console.log("Only one enabled location, auto-selecting: ", only.name);
      settingsDispatch?.({
        type: "set_location",
        id: only.id,
      });
    }
    // Intentionally only react when the number of enabled locations changes
  }, [enabledLocations, selectedLocation, settingsDispatch]);

  if (selectedLocation != null) {
    return <>{children}</>;
  }

  function handleSelectLocation(id: string) {
    settingsDispatch?.({
      type: "set_location",
      id,
    });
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-surface-raised">
      <div className="w-full max-w-150 rounded-lg bg-surface p-10 shadow-md">
        <h1 className="mt-0 mb-2.5 font-title text-3xl text-ink">
          Select Your Location
        </h1>
        <p className="mb-8 text-ink-muted">
          You are logging in for the first time or your location has been reset.
          Please select the unit you would like to administer. You can always
          swap to a different location by clicking the unit name in the menu
          bar.
        </p>

        {enabledLocations.length === 0 ? (
          <p className="rounded-sm bg-red-50 p-2.5 text-red-700">
            No locations available. Please contact an administrator.
          </p>
        ) : (
          <>
            <TextInput
              type="text"
              className="mb-6 box-border p-4 font-title text-lg"
              placeholder="Filter locations…"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              autoFocus
            />
            {filteredLocations.length === 0 ? (
              <p className="py-2 text-center text-ink-muted">
                No locations match “{filter}”.
              </p>
            ) : (
              <OptionList>
                {filteredLocations.map((location) => (
                  <OptionButtonRow
                    key={location.id}
                    onClick={() => handleSelectLocation(location.id)}
                  >
                    <span className="font-title text-lg font-medium text-ink">
                      {location.name}
                    </span>
                  </OptionButtonRow>
                ))}
              </OptionList>
            )}
          </>
        )}
      </div>
    </div>
  );
}

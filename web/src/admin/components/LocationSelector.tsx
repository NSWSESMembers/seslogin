import { useEffect } from "react";
import type { ReactNode } from "react";
import { useSettings, useSettingsDispatch } from "../../lib/settings";
import { useUserInfo } from "./useUserInfo";
import { getLocationById, getSelectedLocationId } from "./useSelectedLocation";
import LocationPicker from "./LocationPicker";

interface LocationSelectorProps {
  children?: ReactNode;
}

export default function LocationSelector({ children }: LocationSelectorProps) {
  const settings = useSettings();
  const settingsDispatch = useSettingsDispatch();
  const { locations } = useUserInfo();
  const enabledLocations = locations
    .filter((loc) => loc.enabled)
    .sort((a, b) => a.name.localeCompare(b.name));
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
    <LocationPicker
      locations={enabledLocations}
      onSelectLocation={handleSelectLocation}
    />
  );
}

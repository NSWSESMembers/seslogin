import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { useSettings, useSettingsDispatch } from "../../lib/settings";
import { useUserInfo } from "./useUserInfo";
import { getLocationById, getSelectedLocationId } from "./useSelectedLocation";
import "../style.css";

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
    <div id="location-selector">
      <div className="selector-container">
        <h1>Select Your Location</h1>
        <p>
          You are logging in for the first time or your location has been reset.
          Please select the unit you would like to administer. You can always
          swap to a different location by clicking the unit name in the menu
          bar.
        </p>

        {enabledLocations.length === 0 ? (
          <p className="error">
            No locations available. Please contact an administrator.
          </p>
        ) : (
          <>
            <input
              type="text"
              className="location-filter"
              placeholder="Filter locations…"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              autoFocus
            />
            {filteredLocations.length === 0 ? (
              <p className="no-results">No locations match “{filter}”.</p>
            ) : (
              <ul className="location-list">
                {filteredLocations.map((location) => (
                  <li key={location.id}>
                    <button
                      onClick={() => handleSelectLocation(location.id)}
                      className="location-button"
                    >
                      <span className="location-name">{location.name}</span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </>
        )}
      </div>
    </div>
  );
}

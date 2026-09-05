import { useState } from "react";
import TextInput from "../../components/ui/TextInput";
import { OptionList, OptionButtonRow } from "../../components/ui/OptionList";

interface LocationPickerProps {
  locations: ReadonlyArray<{ readonly id: string; readonly name: string }>;
  onSelectLocation: (id: string) => void;
}

/**
 * Full-page location chooser. Mounted only while no location is selected, so
 * the filter text is discarded once a location is picked.
 */
export default function LocationPicker({
  locations,
  onSelectLocation,
}: LocationPickerProps) {
  const [filter, setFilter] = useState("");
  const filteredLocations = locations.filter((loc) =>
    loc.name.toLowerCase().includes(filter.trim().toLowerCase()),
  );

  return (
    <div className="flex min-h-screen items-center justify-center bg-surface px-4 py-8 sm:bg-surface-raised sm:px-6 sm:py-12">
      <div className="w-full max-w-150 rounded-none bg-surface p-6 shadow-none sm:rounded-lg sm:p-10 sm:shadow-md">
        <h1 className="mt-0 mb-2.5 font-title text-2xl text-ink sm:text-3xl">
          Select Your Location
        </h1>
        <p className="mb-8 text-ink-muted">
          You are logging in for the first time or your location has been reset.
          Please select the unit you would like to administer. You can always
          swap to a different location by clicking the unit name in the menu
          bar.
        </p>

        {locations.length === 0 ? (
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
                    onClick={() => onSelectLocation(location.id)}
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

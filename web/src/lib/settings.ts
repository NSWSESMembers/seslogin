import { createContext, useContext, type Dispatch } from "react";

export type Settings = {
  locationId: string | null;
};

type SettingsAction =
  { type: "set_location"; id: string } | { type: "clear_location" };

export const SettingsContext = createContext<Settings | null>(null);

export const SettingsDispatchContext =
  createContext<Dispatch<SettingsAction> | null>(null);

export function useSettings() {
  return useContext(SettingsContext);
}

export function useSettingsDispatch() {
  return useContext(SettingsDispatchContext);
}

export function settingsReducer(
  settings: Settings,
  action: SettingsAction,
): Settings {
  switch (action.type) {
    case "set_location": {
      const newSettings = { ...settings };
      newSettings.locationId = action.id;
      return newSettings;
    }
    case "clear_location": {
      const newSettings = { ...settings };
      newSettings.locationId = null;
      return newSettings;
    }
    default: {
      throw Error("Unknown action: " + action["type"]);
    }
  }
}

export const initialSettings: Settings = {
  locationId: null,
};

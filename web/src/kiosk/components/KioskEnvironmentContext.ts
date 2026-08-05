import { createContext } from "react";

/** How this kiosk is authenticating: legacy 6-digit code -> JWT, or a signed key. */
export type KioskAuthMode = "jwt" | "key";

export type KioskEnvironmentContextType = {
  setToken: (token: string) => void;
  /** Which `/kiosk/:profile` this device is running as (`"default"` if unset). */
  profile: string;
  authMode: KioskAuthMode;
};

export const KioskEnvironmentContext = createContext<
  KioskEnvironmentContextType | undefined
>(undefined);

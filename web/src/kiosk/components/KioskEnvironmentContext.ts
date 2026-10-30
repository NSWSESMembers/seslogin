import { createContext } from "react";

export type KioskEnvironmentContextType = {
  /** Which `/kiosk/:profile` this device is running as (`"default"` if unset). */
  profile: string;
  /**
   * Switch this kiosk over to signing with its enrolled key. Called from the
   * enrollment screen once an admin has enrolled the key.
   */
  onKeyEnrolled: () => void;
};

export const KioskEnvironmentContext = createContext<
  KioskEnvironmentContextType | undefined
>(undefined);

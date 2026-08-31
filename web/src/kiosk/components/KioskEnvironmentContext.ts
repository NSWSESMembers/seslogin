import { createContext } from "react";

/** How this kiosk is authenticating: legacy 6-digit code -> JWT, or a signed key. */
export type KioskAuthMode = "jwt" | "key";

export type KioskEnvironmentContextType = {
  setToken: (token: string) => void;
  /** Which `/kiosk/:profile` this device is running as (`"default"` if unset). */
  profile: string;
  authMode: KioskAuthMode;
  /**
   * Switch this kiosk over to signing with its enrolled key, dropping any stored JWT.
   * Called once an admin has enrolled the key — from the enrollment screen, or from
   * the re-enroll section of the status dialog.
   */
  onKeyEnrolled: () => void;
};

export const KioskEnvironmentContext = createContext<
  KioskEnvironmentContextType | undefined
>(undefined);

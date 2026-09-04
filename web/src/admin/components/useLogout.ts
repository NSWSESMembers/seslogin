import { createContext, useContext } from "react";

export const LogoutContext = createContext<(() => void) | null>(null);

/** Clears the seslogin token and returns to the logged-out state. Only callable
 * from inside `AuthenticatedSession` — every area that reuses the admin session
 * gets it this way rather than threading it down as a prop. */
export function useLogout(): () => void {
  const logout = useContext(LogoutContext);
  if (!logout) {
    throw new Error("useLogout must be used within an AuthenticatedSession");
  }
  return logout;
}

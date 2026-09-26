import { useContext } from "react";
import {
  LivePeriodsContext,
  type LivePeriodsContextType,
} from "./LivePeriodsContext";

/**
 * Hook to access the live "who's signed in" list from LivePeriodsProvider.
 * Must be used within a component wrapped by LivePeriodsProvider (mounted for
 * every kiosk route, under KioskSessionProvider).
 */
export function useLivePeriods(): LivePeriodsContextType {
  const context = useContext(LivePeriodsContext);
  if (context === undefined) {
    throw new Error("useLivePeriods must be used within a LivePeriodsProvider");
  }
  return context;
}

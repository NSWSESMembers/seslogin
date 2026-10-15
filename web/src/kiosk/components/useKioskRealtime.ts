import { useContext } from "react";
import {
  KioskRealtimeContext,
  type KioskRealtimeContextType,
} from "./KioskRealtimeContext";

/**
 * Hook to access the kiosk's shared Ably connection from KioskRealtimeProvider
 * (mounted under KioskSessionProvider for every kiosk route).
 */
export function useKioskRealtime(): KioskRealtimeContextType {
  const context = useContext(KioskRealtimeContext);
  if (context === undefined) {
    throw new Error(
      "useKioskRealtime must be used within a KioskRealtimeProvider",
    );
  }
  return context;
}

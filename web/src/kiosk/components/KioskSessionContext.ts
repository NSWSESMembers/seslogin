import { createContext } from "react";

export type JsonValue =
  string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue };

export interface KioskSession {
  id: string;
  name: string;
  config: { [key: string]: JsonValue };
  /** The session record's version: bumped by each admin edit, so a push or
   * refetch older than what the kiosk already shows can be told apart. */
  version: number;
  location: {
    id: string;
    name: string;
  };
}

export interface KioskSessionContextType {
  session: KioskSession | null;
}

export const KioskSessionContext = createContext<
  KioskSessionContextType | undefined
>(undefined);

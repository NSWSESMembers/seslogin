import { createContext } from "react";

export type JsonValue =
  string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue };

export interface KioskSession {
  id: string;
  name: string;
  config: { [key: string]: JsonValue };
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

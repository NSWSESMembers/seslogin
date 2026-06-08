import { createContext, useContext } from "react";

export type ToastKind = "error" | "success";

export type Toast = {
  id: number;
  message: string;
  kind: ToastKind;
};

export type NotifyContextValue = {
  /** Show a plain message toast (defaults to an error toast). */
  notify: (message: string, kind?: ToastKind) => void;
  /** Show a green success toast, e.g. after a mutation completes. */
  notifySuccess: (message: string) => void;
  /**
   * Show an error toast built from a caught error / rejected mutation. Extracts
   * the server's error detail and optionally prefixes it with context, e.g.
   * notifyError(err, "Couldn't delete member").
   */
  notifyError: (err: unknown, prefix?: string) => void;
  dismiss: (id: number) => void;
};

export const NotifyContext = createContext<NotifyContextValue | null>(null);

export function useNotify(): NotifyContextValue {
  const ctx = useContext(NotifyContext);
  if (!ctx) {
    throw new Error("useNotify must be used within a NotificationProvider");
  }
  return ctx;
}

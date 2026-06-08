import { useCallback, useMemo, useRef, useState, type ReactNode } from "react";
import { getErrorMessage } from "../../lib/relayErrors";
import { NotifyContext, type Toast, type ToastKind } from "./useNotify";

const AUTO_DISMISS_MS = 10_000;

export function NotificationProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(0);

  const dismiss = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const notify = useCallback(
    (message: string, kind: ToastKind = "error") => {
      const id = nextId.current++;
      setToasts((prev) => [...prev, { id, message, kind }]);
      setTimeout(() => dismiss(id), AUTO_DISMISS_MS);
    },
    [dismiss],
  );

  const notifySuccess = useCallback(
    (message: string) => notify(message, "success"),
    [notify],
  );

  const notifyError = useCallback(
    (err: unknown, prefix?: string) => {
      const detail = getErrorMessage(err);
      notify(prefix ? `${prefix}: ${detail}` : detail, "error");
      // Keep the full error in the console for debugging.
      console.error(prefix ?? "Error", err);
    },
    [notify],
  );

  const value = useMemo(
    () => ({ notify, notifySuccess, notifyError, dismiss }),
    [notify, notifySuccess, notifyError, dismiss],
  );

  return (
    <NotifyContext.Provider value={value}>
      {children}
      <div className="toast-region" role="region" aria-label="Notifications">
        {toasts.map((t) => (
          <div key={t.id} className={`toast toast--${t.kind}`} role="alert">
            <span className="toast__message">{t.message}</span>
            <button
              type="button"
              className="toast__close"
              aria-label="Dismiss"
              onClick={() => dismiss(t.id)}
            >
              ×
            </button>
          </div>
        ))}
      </div>
    </NotifyContext.Provider>
  );
}

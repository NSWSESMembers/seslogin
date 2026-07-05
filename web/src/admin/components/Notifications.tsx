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

  const kindClasses: Record<ToastKind, string> = {
    error: "border border-red-900/25 bg-red-50 text-red-900",
    success: "border border-green-800/25 bg-green-50 text-green-900",
  };

  return (
    <NotifyContext.Provider value={value}>
      {children}
      <div
        className="fixed top-4 right-4 z-1000 flex max-w-[min(420px,calc(100vw-32px))] flex-col gap-2.5"
        role="region"
        aria-label="Notifications"
      >
        {toasts.map((t) => (
          <div
            key={t.id}
            className={`flex items-start gap-3 rounded-xl px-3.5 py-3 text-[15px] leading-snug shadow-lg ${kindClasses[t.kind]}`}
            role="alert"
          >
            <span className="flex-1 wrap-break-word">{t.message}</span>
            <button
              type="button"
              className="flex-none cursor-pointer border-0 bg-transparent p-0 text-xl leading-none text-inherit"
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

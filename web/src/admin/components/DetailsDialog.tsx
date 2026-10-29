import { Suspense, useEffect, type ReactNode } from "react";
import { Dialog, DialogActions, DialogTitle } from "../../components/ui/Dialog";
import { Button } from "../../components/ui/Button";
import LoadingIndicator from "../../components/LoadingIndicator";
import RelayErrorBoundary from "../../components/RelayErrorBoundary";

// Shell shared by the read-only member and unit details panels: a dismissible
// dialog (backdrop click, Close button, Escape) whose body does its own lazy
// query, so opening it is the only thing that costs a request.
export function DetailsDialog({
  title,
  onClose,
  children,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
}) {
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return (
    <Dialog onDismiss={onClose} width="w-160">
      <DialogTitle>{title}</DialogTitle>
      <div className="text-left" role="region" aria-label={title}>
        <RelayErrorBoundary canRetry>
          <Suspense fallback={<LoadingIndicator />}>{children}</Suspense>
        </RelayErrorBoundary>
      </div>
      <DialogActions>
        <Button variant="secondary" onClick={onClose}>
          Close
        </Button>
      </DialogActions>
    </Dialog>
  );
}

// A labelled value; renders nothing for a missing one so empty rows disappear.
export function Field({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <div className="grid grid-cols-[9rem_1fr] gap-x-3 py-1">
      <dt className="text-sm font-semibold text-ink-strong">{label}</dt>
      <dd className="m-0 wrap-break-word">{children}</dd>
    </div>
  );
}

export function Section({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <section className="mt-4">
      <h3 className="m-0 mb-1 border-b border-line pb-1 font-title text-navy">
        {title}
      </h3>
      {children}
    </section>
  );
}

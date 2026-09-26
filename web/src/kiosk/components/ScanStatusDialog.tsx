import { useEffect } from "react";
import { Button } from "../../components/ui/Button";
import { Dialog, DialogActions, DialogTitle } from "../../components/ui/Dialog";
import { formatTime, formatTimeDiff } from "../../lib/time";
import { signInColorClass } from "../lib/signInColor";
import { useLivePeriods } from "./useLivePeriods";

// A kiosk left showing this list is a kiosk not showing the scan prompt, so it
// closes itself. Scanning also closes it (see ScanController), which is the
// usual way out.
const AUTO_CLOSE_MS = 60_000;

/**
 * Who is currently signed in at this location, shown on top of the scan screen.
 * Deliberately read-only — signing someone else out is an admin action, not
 * something a kiosk hands to whoever walks past.
 *
 * Unlike the other scan overlays this one takes no scan-focus lease: the member
 * ID input underneath keeps focus, so a scan still registers while the list is
 * up and closes it on the way through.
 */
export default function ScanStatusDialog(props: { onClose: () => void }) {
  const { onClose } = props;
  const { periods, loading, error, retry } = useLivePeriods();

  useEffect(() => {
    const timeoutId = window.setTimeout(onClose, AUTO_CLOSE_MS);
    return () => window.clearTimeout(timeoutId);
  }, [onClose]);

  return (
    <Dialog onDismiss={onClose} width="w-175">
      <DialogTitle>Currently signed in</DialogTitle>
      {loading ? (
        <p className="m-0 text-ink-muted">Loading…</p>
      ) : error ? (
        <div className="flex flex-col items-start gap-2">
          <p className="m-0 text-red-600">Couldn't load the list.</p>
          <Button variant="kiosk" onClick={retry}>
            Try again
          </Button>
        </div>
      ) : periods.length === 0 ? (
        <p className="m-0 text-ink-muted">
          Nobody is currently signed in here.
        </p>
      ) : (
        <>
          <ul className="m-0 flex max-h-[60vh] list-none flex-col gap-1 overflow-y-auto p-0">
            {periods.map((entry) => (
              <li
                key={entry.id}
                className="flex items-baseline justify-between gap-4 border-b border-line py-1"
              >
                <span className="min-w-0 font-bold">{entry.name}</span>
                <span className="shrink-0 text-right">
                  <span className="text-ink-muted">
                    since {formatTime(new Date(entry.startTime * 1000))}
                  </span>{" "}
                  <span className={signInColorClass(entry.startTime)}>
                    (
                    {formatTimeDiff(
                      new Date(entry.startTime * 1000),
                      new Date(),
                    )}
                    )
                  </span>
                </span>
              </li>
            ))}
          </ul>
          <p className="m-0 font-bold">{periods.length} signed in</p>
        </>
      )}
      <DialogActions>
        <Button variant="kiosk" onClick={onClose}>
          Close
        </Button>
      </DialogActions>
    </Dialog>
  );
}

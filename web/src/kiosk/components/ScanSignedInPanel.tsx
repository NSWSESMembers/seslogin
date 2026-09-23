import { formatTimeDiff } from "../../lib/time";
import { signInColorClass } from "../lib/signInColor";
import { Button } from "../../components/ui/Button";
import { useLivePeriods } from "./useLivePeriods";

/**
 * Always-on companion to ScanStatusDialog: the same signed-in list, but
 * embedded permanently next to the member ID input instead of behind a
 * button — enabled by the "Always visible" setting of the Who's here
 * option (see SessionForm), which is mutually exclusive with the button in
 * practice: a button that opens the list you're already looking at would be
 * pointless, so ScanController hides it when this is on.
 *
 * Reads-only, like the dialog, and doesn't need to react to scans the way the
 * dialog does — it's laid out beside the member ID input rather than over it,
 * so it never has a scan result to get out of the way of.
 */
export default function ScanSignedInPanel() {
  const { periods, loading, error, retry } = useLivePeriods();

  return (
    <div className="flex h-full flex-col">
      <h2 className="m-0 mb-2 text-left text-lg font-bold">
        Currently signed in
      </h2>
      {loading ? (
        <p className="m-0 text-base text-ink-muted">Loading…</p>
      ) : error ? (
        <div className="flex flex-1 flex-col items-start gap-2">
          <p className="m-0 text-base text-red-600">Couldn't load the list.</p>
          <Button variant="kiosk" onClick={retry}>
            Try again
          </Button>
        </div>
      ) : periods.length === 0 ? (
        <p className="m-0 text-base text-ink-muted">Nobody signed in here.</p>
      ) : (
        <>
          <ul className="m-0 flex flex-1 list-none flex-col gap-0.5 overflow-y-auto p-0 text-base">
            {periods.map((entry) => (
              <li
                key={entry.id}
                className="flex items-baseline justify-between gap-3"
              >
                <span className="min-w-0 truncate text-left">{entry.name}</span>
                <span
                  className={`shrink-0 text-right ${signInColorClass(entry.startTime)}`}
                >
                  {formatTimeDiff(new Date(entry.startTime * 1000), new Date())}
                </span>
              </li>
            ))}
          </ul>
          <p className="m-0 mt-auto pt-2 text-base font-bold">
            {periods.length} signed in
          </p>
        </>
      )}
    </div>
  );
}

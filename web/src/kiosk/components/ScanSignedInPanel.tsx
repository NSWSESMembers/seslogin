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
 * Read-only unless `onSignOut` is given (the "Visible + Sign out" setting).
 * Unlike the dialog it doesn't need to react to scans: it's laid out beside the
 * member ID input rather than over it, so it never has a scan result to get out
 * of the way of.
 */
export default function ScanSignedInPanel(props: {
  /** Start the usual sign-out steps for this member. When set, each member
   * row gets a Sign out button; guests keep using the guest dialog. */
  onSignOut?: (periodId: string, name: string) => void;
  /** Greys the buttons out while a sign-out is already under way. */
  signOutDisabled?: boolean;
}) {
  const { onSignOut, signOutDisabled } = props;
  const { periods, guests, loading, error, retry } = useLivePeriods();
  const guestIds = new Set(guests.map((guest) => guest.id));
  // With sign out on, a name keeps its natural width and only when it can't sit
  // beside the time and button (a narrow kiosk, a long name) do those drop to a
  // second line; a name too long even for the panel wraps rather than being cut
  // off. Without it the row is the original single, truncating line.
  const rowClassName = onSignOut
    ? "flex flex-wrap items-center justify-between gap-x-3 gap-y-1"
    : "flex items-baseline justify-between gap-3";
  const nameClassName = onSignOut
    ? "max-w-full text-left wrap-break-word"
    : "min-w-0 truncate text-left";

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
              <li key={entry.id} className={rowClassName}>
                <span className={nameClassName}>{entry.name}</span>
                <span className="ml-auto flex shrink-0 items-center gap-2">
                  <span
                    className={`text-right ${signInColorClass(entry.startTime)}`}
                  >
                    {formatTimeDiff(
                      new Date(entry.startTime * 1000),
                      new Date(),
                    )}
                  </span>
                  {onSignOut && !guestIds.has(entry.id) && (
                    <Button
                      variant="kiosk"
                      type="button"
                      // A kiosk is tapped, not clicked: keep the target at the
                      // 44px touch minimum rather than the 32px default.
                      className="min-h-11"
                      disabled={signOutDisabled}
                      aria-label={`Sign out ${entry.name}`}
                      onClick={() => onSignOut(entry.id, entry.name)}
                    >
                      Sign out
                    </Button>
                  )}
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

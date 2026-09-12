import { Suspense, useEffect, useState } from "react";
import { graphql } from "react-relay";
import { Button } from "../../components/ui/Button";
import { Dialog, DialogActions, DialogTitle } from "../../components/ui/Dialog";
import { formatTime, formatTimeDiff } from "../../lib/time";
import { signInColorClass } from "../lib/signInColor";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import type { ScanStatusDialogQuery } from "./__generated__/ScanStatusDialogQuery.graphql";

// Long enough that the list is still current while someone reads it, short
// enough that it isn't worth a manual refresh button on a touchscreen nobody
// wants to poke twice.
const REFRESH_INTERVAL_MS = 30_000;
// A kiosk left showing this list is a kiosk not showing the scan prompt, so it
// closes itself. Scanning also closes it (see ScanController), which is the
// usual way out.
const AUTO_CLOSE_MS = 60_000;
// Matches the full-screen status kiosk's own cap (pages/Status.tsx) so the two
// can't report different totals for the same room.
const MAX_PERIODS = 100;

type SignedIn = {
  id: string;
  startTime: number;
  name: string;
};

function SignedInList(props: { refreshKey: number }) {
  const data = useRetryableLazyLoadQuery<ScanStatusDialogQuery>(
    graphql`
      query ScanStatusDialogQuery($first: Int!) @throwOnFieldError {
        session {
          location {
            periods(onlyActive: true, first: $first) {
              edges {
                node {
                  id
                  startTime
                  guestName
                  person {
                    id
                    firstName
                    lastName
                  }
                }
              }
            }
          }
        }
      }
    `,
    { first: MAX_PERIODS },
    { fetchPolicy: "network-only", fetchKey: props.refreshKey },
  );

  const signedIn: SignedIn[] = data.session.location.periods.edges
    .filter((edge): edge is NonNullable<typeof edge> => edge?.node != null)
    .map(({ node }) => ({
      id: node.id,
      startTime: node.startTime,
      name: node.person
        ? `${node.person.firstName} ${node.person.lastName}`
        : `${node.guestName ?? "Guest"} (Guest)`,
    }))
    // Longest signed in first: the people most likely to have forgotten to sign
    // out are the reason to look at this list at all.
    .sort((a, b) => a.startTime - b.startTime);

  if (signedIn.length === 0) {
    return (
      <p className="m-0 text-ink-muted">Nobody is currently signed in here.</p>
    );
  }

  return (
    <>
      <ul className="m-0 flex max-h-[60vh] list-none flex-col gap-1 overflow-y-auto p-0">
        {signedIn.map((entry) => (
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
                ({formatTimeDiff(new Date(entry.startTime * 1000), new Date())})
              </span>
            </span>
          </li>
        ))}
      </ul>
      <p className="m-0 font-bold">{signedIn.length} signed in</p>
    </>
  );
}

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
  const [refreshKey, setRefreshKey] = useState(0);

  useEffect(() => {
    const intervalId = window.setInterval(() => {
      setRefreshKey((k) => k + 1);
    }, REFRESH_INTERVAL_MS);
    return () => window.clearInterval(intervalId);
  }, []);

  useEffect(() => {
    const timeoutId = window.setTimeout(onClose, AUTO_CLOSE_MS);
    return () => window.clearTimeout(timeoutId);
  }, [onClose]);

  return (
    <Dialog onDismiss={onClose} width="w-175">
      <DialogTitle>Currently signed in</DialogTitle>
      <Suspense fallback={<p className="m-0 text-ink-muted">Loading…</p>}>
        <SignedInList refreshKey={refreshKey} />
      </Suspense>
      <DialogActions>
        <Button variant="kiosk" onClick={onClose}>
          Close
        </Button>
      </DialogActions>
    </Dialog>
  );
}

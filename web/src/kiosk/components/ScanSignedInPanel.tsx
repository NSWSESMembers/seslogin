import { Suspense, useEffect, useState } from "react";
import { graphql } from "react-relay";
import { formatTimeDiff } from "../../lib/time";
import { signInColorClass } from "../lib/signInColor";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import type { ScanSignedInPanelQuery } from "./__generated__/ScanSignedInPanelQuery.graphql";

// Same cadence as ScanStatusDialog's on-demand list — frequent enough that a
// screen left showing this panel stays current, without hammering the API
// from every kiosk in the fleet that enables it.
const REFRESH_INTERVAL_MS = 30_000;
// Matches the full-screen status kiosk's own cap (pages/Status.tsx) and
// ScanStatusDialog's, so no two of the three can report a different total for
// the same room.
const MAX_PERIODS = 100;

type SignedIn = {
  id: string;
  startTime: number;
  name: string;
};

function SignedInPanelList(props: { refreshKey: number }) {
  const data = useRetryableLazyLoadQuery<ScanSignedInPanelQuery>(
    graphql`
      query ScanSignedInPanelQuery($first: Int!) @throwOnFieldError {
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
    // Longest signed in first: whoever might have forgotten to sign out is the
    // most useful thing to see without pressing anything.
    .sort((a, b) => a.startTime - b.startTime);

  if (signedIn.length === 0) {
    return <p className="m-0 text-sm text-ink-muted">Nobody signed in here.</p>;
  }

  return (
    <>
      <ul className="m-0 flex max-h-[55vh] list-none flex-col gap-1 overflow-y-auto p-0 text-sm">
        {signedIn.map((entry) => (
          <li
            key={entry.id}
            className="flex items-baseline justify-between gap-3 py-0.5"
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
      <p className="m-0 mt-2 text-sm font-bold">{signedIn.length} signed in</p>
    </>
  );
}

/**
 * Always-on companion to ScanStatusDialog: the same signed-in list, but
 * embedded permanently next to the member ID input instead of behind a
 * button — enabled by the "Always visible" setting of the Who's signed in
 * option (see SessionForm), which is mutually exclusive with the button in
 * practice: a button that opens the list you're already looking at would be
 * pointless, so ScanController hides it when this is on.
 *
 * Reads-only, like the dialog, and doesn't need to react to scans the way the
 * dialog does — it's laid out beside the member ID input rather than over it,
 * so it never has a scan result to get out of the way of.
 */
export default function ScanSignedInPanel() {
  const [refreshKey, setRefreshKey] = useState(0);

  useEffect(() => {
    const intervalId = window.setInterval(() => {
      setRefreshKey((k) => k + 1);
    }, REFRESH_INTERVAL_MS);
    return () => window.clearInterval(intervalId);
  }, []);

  return (
    <div>
      <h2 className="m-0 mb-2 text-left text-lg font-bold">
        Currently signed in
      </h2>
      <Suspense
        fallback={<p className="m-0 text-sm text-ink-muted">Loading…</p>}
      >
        <SignedInPanelList refreshKey={refreshKey} />
      </Suspense>
    </div>
  );
}

import { Suspense, useState } from "react";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import { inputBase } from "../../components/ui/inputStyles";
import { Button } from "../../components/ui/Button";
import { formatTime } from "../../lib/time";
import type { ScanGuestDialogQuery } from "./__generated__/ScanGuestDialogQuery.graphql";
import type { ScanGuestDialogSignInMutation } from "./__generated__/ScanGuestDialogSignInMutation.graphql";
import type { ScanGuestDialogSignOutMutation } from "./__generated__/ScanGuestDialogSignOutMutation.graphql";

// Currently-signed-in guests, fetched fresh each time the list is shown. Rendered
// inside <Suspense> so the surrounding dialog stays interactive while it loads.
function GuestList(props: {
  onSignOut: (id: string) => void;
  signOutInFlightId: string | null;
}) {
  const data = useLazyLoadQuery<ScanGuestDialogQuery>(
    graphql`
      query ScanGuestDialogQuery($first: Int!) {
        session {
          location {
            periods(onlyActive: true, first: $first) {
              edges {
                node {
                  id
                  startTime
                  guestName
                }
              }
            }
          }
        }
      }
    `,
    { first: 100 },
    { fetchPolicy: "network-only" },
  );

  const guests = data.session.location.periods.edges
    .filter((edge): edge is NonNullable<typeof edge> => edge?.node != null)
    .map((edge) => edge.node)
    .filter((node) => node.guestName != null);

  if (guests.length === 0) {
    return <p className="text-ink-muted">No guests are currently signed in.</p>;
  }

  return (
    <ul className="m-0 flex list-none flex-col gap-2 p-0">
      {guests.map((guest) => (
        <li
          key={guest.id}
          className="flex items-center justify-between gap-4 border-b border-line py-1"
        >
          <span>
            <span className="font-bold">{guest.guestName}</span>{" "}
            <span className="text-ink-muted">
              signed in at {formatTime(new Date(guest.startTime * 1000))}
            </span>
          </span>
          <Button
            variant="kiosk"
            onClick={() => props.onSignOut(guest.id)}
            disabled={props.signOutInFlightId !== null}
          >
            {props.signOutInFlightId === guest.id ? "Signing out…" : "Sign out"}
          </Button>
        </li>
      ))}
    </ul>
  );
}

export default function ScanGuestDialog(props: { onClose: () => void }) {
  const { onClose } = props;

  const [name, setName] = useState("");
  const [reason, setReason] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [signOutInFlightId, setSignOutInFlightId] = useState<string | null>(
    null,
  );

  const [commitSignIn, signInInFlight] =
    useMutation<ScanGuestDialogSignInMutation>(graphql`
      mutation ScanGuestDialogSignInMutation($name: String!, $reason: String) {
        scanGuestSignIn(name: $name, reason: $reason) {
          id
          startTime
          guestName
        }
      }
    `);

  const [commitSignOut] = useMutation<ScanGuestDialogSignOutMutation>(graphql`
    mutation ScanGuestDialogSignOutMutation($id: ID!) {
      scanGuestSignOut(id: $id) {
        id
        endTime
      }
    }
  `);

  const trimmedName = name.trim();

  function handleSignIn() {
    if (trimmedName === "" || signInInFlight) {
      return;
    }
    setError(null);
    const trimmedReason = reason.trim();
    commitSignIn({
      variables: {
        name: trimmedName,
        reason: trimmedReason === "" ? null : trimmedReason,
      },
      onCompleted: () => onClose(),
      onError: (err) => {
        console.error("Guest sign-in failed:", err);
        setError("Couldn't sign in the guest. Please try again.");
      },
    });
  }

  function handleSignOut(id: string) {
    if (signOutInFlightId !== null) {
      return;
    }
    setError(null);
    setSignOutInFlightId(id);
    commitSignOut({
      variables: { id },
      onCompleted: () => onClose(),
      onError: (err) => {
        console.error("Guest sign-out failed:", err);
        setSignOutInFlightId(null);
        setError("Couldn't sign out the guest. Please try again.");
      },
    });
  }

  return (
    <div className="fixed inset-0 z-10 flex items-center justify-center">
      <div
        className="absolute inset-0 bg-black opacity-50"
        onClick={onClose}
      ></div>
      <div className="relative z-10 flex w-150 max-w-[90vw] flex-col gap-4 rounded-xl bg-surface p-6 shadow-2xl">
        <h2 className="m-0 text-2xl font-bold">Guest sign in / out</h2>

        {error && <p className="m-0 font-bold text-red-600">{error}</p>}

        <div className="flex flex-col gap-3">
          <label className="flex flex-col gap-1">
            <span>Name</span>
            <input
              type="text"
              className={inputBase}
              value={name}
              maxLength={100}
              required
              autoFocus
              onChange={(e) => setName(e.target.value)}
            />
          </label>
          <label className="flex flex-col gap-1">
            <span>Reason for visit (optional)</span>
            <input
              type="text"
              className={inputBase}
              value={reason}
              maxLength={500}
              onChange={(e) => setReason(e.target.value)}
            />
          </label>
          <Button
            variant="kiosk"
            onClick={handleSignIn}
            disabled={trimmedName === "" || signInInFlight}
          >
            {signInInFlight ? "Signing in…" : "Sign in"}
          </Button>
        </div>

        <div className="flex flex-col gap-2">
          <h3 className="m-0 text-lg font-bold">Currently signed in guests</h3>
          <Suspense fallback={<p className="text-ink-muted">Loading…</p>}>
            <GuestList
              onSignOut={handleSignOut}
              signOutInFlightId={signOutInFlightId}
            />
          </Suspense>
        </div>

        <div className="flex justify-end">
          <Button variant="secondary" onClick={onClose}>
            Cancel
          </Button>
        </div>
      </div>
    </div>
  );
}

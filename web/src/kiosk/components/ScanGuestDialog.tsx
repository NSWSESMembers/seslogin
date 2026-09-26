import { useState } from "react";
import { graphql, useMutation } from "react-relay";
import { inputBase } from "../../components/ui/inputStyles";
import { Button } from "../../components/ui/Button";
import { Dialog, DialogActions, DialogTitle } from "../../components/ui/Dialog";
import { StatusMessage } from "../../components/ui/StatusMessage";
import { formatTime } from "../../lib/time";
import { getServerErrorMessage } from "../../lib/relayErrors";
import type { ScanGuestDialogSignInMutation } from "./__generated__/ScanGuestDialogSignInMutation.graphql";
import type { ScanGuestDialogSignOutMutation } from "./__generated__/ScanGuestDialogSignOutMutation.graphql";
import { useSuspendScanFocus } from "../lib/scanFocusLeases";
import { useLivePeriods } from "./useLivePeriods";

const GUEST_DIALOG_FOCUS_LEASE_ID = "scan:guest-dialog";

// Currently-signed-in guests, from the shared live list (LivePeriodsProvider) —
// updated over Ably rather than fetched fresh each time the dialog opens.
function GuestList(props: {
  onSignOut: (id: string) => void;
  signOutInFlightId: string | null;
}) {
  const { guests, loading, error, retry } = useLivePeriods();

  if (loading) {
    return <p className="my-4 text-ink-muted">Loading…</p>;
  }

  if (error) {
    return (
      <div className="my-4 flex flex-col items-start gap-2">
        <p className="m-0 text-red-600">Couldn't load the guest list.</p>
        <Button variant="kiosk" onClick={retry}>
          Try again
        </Button>
      </div>
    );
  }

  if (guests.length === 0) {
    return (
      <p className="my-4 text-ink-muted">No guests are currently signed in.</p>
    );
  }

  return (
    <ul className="m-0 flex list-none flex-col gap-2 p-0">
      {guests.map((guest) => (
        <li
          key={guest.id}
          className="flex items-center justify-between gap-4 border-b border-line py-1"
        >
          <span>
            <span className="font-bold">{guest.name}</span>{" "}
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
  const { applyOwnResult } = useLivePeriods();

  // Only mounted while the dialog is open, so the lease lasts exactly that long.
  useSuspendScanFocus(GUEST_DIALOG_FOCUS_LEASE_ID);

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
          version
          startTime
          guestName
        }
      }
    `);

  const [commitSignOut] = useMutation<ScanGuestDialogSignOutMutation>(graphql`
    mutation ScanGuestDialogSignOutMutation($id: ID!) {
      scanGuestSignOut(id: $id) {
        id
        version
        startTime
        endTime
        guestName
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
      onCompleted: (data) => {
        const period = data.scanGuestSignIn;
        applyOwnResult({
          kind: "opened",
          periodId: period.id,
          version: period.version,
          name: period.guestName ?? trimmedName,
          guest: true,
          startTime: period.startTime,
        });
        onClose();
      },
      onError: (err) => {
        console.error("Guest sign-in failed:", err);
        const serverMessage = getServerErrorMessage(err);
        setError(
          serverMessage
            ? `Couldn't sign in the guest: ${serverMessage}`
            : "Couldn't sign in the guest. Please try again.",
        );
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
      onCompleted: (data) => {
        const period = data.scanGuestSignOut;
        applyOwnResult({
          kind: "closed",
          periodId: period.id,
          version: period.version,
          name: period.guestName ?? "Guest",
          guest: true,
          startTime: period.startTime,
          endTime: period.endTime ?? null,
          deleted: false,
        });
        onClose();
      },
      onError: (err) => {
        console.error("Guest sign-out failed:", err);
        setSignOutInFlightId(null);
        const serverMessage = getServerErrorMessage(err);
        setError(
          serverMessage
            ? `Couldn't sign out the guest: ${serverMessage}`
            : "Couldn't sign out the guest. Please try again.",
        );
      },
    });
  }

  return (
    <Dialog onDismiss={onClose}>
      <DialogTitle>Guest sign in / out</DialogTitle>

      {error && (
        <StatusMessage variant="error" className="m-0">
          {error}
        </StatusMessage>
      )}

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
        <GuestList
          onSignOut={handleSignOut}
          signOutInFlightId={signOutInFlightId}
        />
      </div>

      <DialogActions>
        <Button variant="secondary" onClick={onClose}>
          Cancel
        </Button>
      </DialogActions>
    </Dialog>
  );
}

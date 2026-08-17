import { useState } from "react";
import { formatSeconds } from "../../lib/time";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import SessionStatus from "../components/SessionStatus";
import useSelectedLocation from "../components/useSelectedLocation";
import { useUserInfo } from "../components/useUserInfo";
import bulletGreen from "../../assets/bullet-green.svg";
import bulletOrange from "../../assets/bullet-orange.svg";
import bulletRed from "../../assets/bullet-red.svg";
import bulletGray from "../../assets/bullet-gray.svg";
import type {
  SessionsListQuery,
  SessionsListQuery$data,
} from "./__generated__/SessionsListQuery.graphql";
import type { SessionsListDeleteMutation } from "./__generated__/SessionsListDeleteMutation.graphql";
import type { SessionsListReactivateMutation } from "./__generated__/SessionsListReactivateMutation.graphql";
import { useNotify } from "../components/useNotify";
import { AdminTable, Th, Td } from "../../components/ui/Table";
import { Button, ButtonLink } from "../../components/ui/Button";
import { Dialog, DialogActions, DialogTitle } from "../../components/ui/Dialog";

type Session = SessionsListQuery$data["location"]["sessions"][number];

// The kiosk prints the same 16-hex-digit prefix of its key fingerprint under the
// enrollment QR code (see KioskEnrollment), so showing the identical truncation here
// lets an admin compare the two by eye.
function shortFingerprint(fingerprint: string) {
  return `${fingerprint.slice(0, 16)}…`;
}

function Row({
  session,
  idx,
  isDev,
}: {
  session: Session;
  idx: number;
  isDev: boolean;
}) {
  const [now] = useState(() => Math.round(Date.now() / 1000));
  const [confirmingReactivate, setConfirmingReactivate] = useState(false);
  const { notifyError, notifySuccess } = useNotify();
  const [commitMutation, isMutationInFlight] =
    useMutation<SessionsListDeleteMutation>(graphql`
      mutation SessionsListDeleteMutation($id: ID!) {
        deleteSession(id: $id)
      }
    `);
  // Selecting the same fields the list does lets Relay normalise the response straight
  // into the store, so the row updates itself without an explicit updater.
  const [commitReactivate, isReactivateInFlight] =
    useMutation<SessionsListReactivateMutation>(graphql`
      mutation SessionsListReactivateMutation($id: ID!) {
        reactivateSession(id: $id) {
          id
          keyEnrolled
          keyExpiresAt
          reactivatable
          lastContact
        }
      }
    `);

  async function deleteSession() {
    const yes = confirm(
      `Are you sure you want to delete this kiosk? Any computer using it will no longer be able to be used to access the system. This action cannot be undone.`,
    );
    if (yes) {
      try {
        await new Promise((resolve, reject) => {
          commitMutation({
            variables: { id: session.id },
            onCompleted: resolve,
            onError: reject,
            updater: (store) => {
              store.delete(session.id);
            },
          });
        });
        notifySuccess(`Kiosk ${session.name} deleted`);
      } catch (err) {
        notifyError(err, `Couldn't delete kiosk ${session.name}`);
      }
    }
  }

  async function reactivateSession() {
    setConfirmingReactivate(false);
    try {
      await new Promise((resolve, reject) => {
        commitReactivate({
          variables: { id: session.id },
          onCompleted: resolve,
          onError: reject,
        });
      });
      notifySuccess(
        `Kiosk ${session.name} reactivated — it should come back online within a few minutes`,
      );
    } catch (err) {
      notifyError(err, `Couldn't reactivate kiosk ${session.name}`);
    }
  }

  // An expired kiosk that isn't currently asking to be re-enrolled can't be reactivated
  // yet — it has to be switched on first.
  const isExpiredKiosk =
    session.keyEnrolled &&
    session.keyExpiresAt != null &&
    session.keyExpiresAt <= now;

  const timeSinceAccess = session.lastContact
    ? formatSeconds(now - session.lastContact)
    : "never";

  // cap client version length to 7 chars
  const clientVersion = session.clientVersion
    ? session.clientVersion.length > 7
      ? session.clientVersion.slice(0, 7)
      : session.clientVersion
    : "-";

  return (
    <tr className={idx % 2 === 0 ? "bg-surface-raised" : undefined}>
      <Td center>
        <SessionStatus lastContact={session.lastContact} />
      </Td>
      {isDev && <Td className="font-mono text-[0.85em]">{session.id}</Td>}
      <Td>{session.name}</Td>
      <Td>{timeSinceAccess}</Td>
      <Td>{session.code}</Td>
      <Td
        className="font-mono text-[0.85em]"
        title={session.keyFingerprint ?? undefined}
      >
        {session.keyFingerprint
          ? shortFingerprint(session.keyFingerprint)
          : "-"}
      </Td>
      <Td>{clientVersion}</Td>
      <Td options>
        <div className="flex items-center justify-end gap-1">
          {session.reactivatable ? (
            <Button
              size="row"
              onClick={() => setConfirmingReactivate(true)}
              disabled={isReactivateInFlight}
            >
              Reactivate
            </Button>
          ) : isExpiredKiosk ? (
            <span
              className="mr-1 text-xs text-ink-muted"
              title="Switch this kiosk on and wait for it to show its QR code, then reload this page to reactivate it."
            >
              Expired
            </span>
          ) : null}
          <ButtonLink size="row" to={`/admin/sessions/${session.id}`}>
            Edit
          </ButtonLink>
          <Button
            size="row"
            variant="danger"
            onClick={deleteSession}
            disabled={isMutationInFlight}
          >
            Delete
          </Button>
        </div>
        {confirmingReactivate && (
          <ReactivateDialog
            session={session}
            onConfirm={reactivateSession}
            onCancel={() => setConfirmingReactivate(false)}
          />
        )}
      </Td>
    </tr>
  );
}

// Reactivation grants a lapsed key a fresh window, so it has to be the right device:
// the admin checks the fingerprint here against the one on the kiosk's QR screen before
// confirming.
function ReactivateDialog({
  session,
  onConfirm,
  onCancel,
}: {
  session: Session;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <Dialog onDismiss={onCancel} width="w-120">
      <DialogTitle>Reactivate {session.name}?</DialogTitle>
      <p className="m-0 text-left">
        Check the device key shown under the QR code on the kiosk screen. It
        must match:
      </p>
      <p className="m-0 text-center font-mono text-lg break-all">
        {session.keyFingerprint
          ? shortFingerprint(session.keyFingerprint)
          : "-"}
      </p>
      <p className="m-0 text-left text-sm text-ink-muted">
        If it doesn't match, cancel — you would be reactivating a different
        computer.
      </p>
      <DialogActions>
        <Button variant="secondary" onClick={onCancel}>
          Cancel
        </Button>
        <Button onClick={onConfirm}>Reactivate</Button>
      </DialogActions>
    </Dialog>
  );
}

export default function SessionsList() {
  const { isDev } = useUserInfo();
  const selectedLocation = useSelectedLocation();
  const locationId = selectedLocation.id;
  const data = useLazyLoadQuery<SessionsListQuery>(
    graphql`
      query SessionsListQuery($location: ID!) {
        location(id: $location) {
          id
          sessions {
            id
            name
            code
            lastContact
            clientVersion
            keyEnrolled
            keyExpiresAt
            keyFingerprint
            reactivatable
          }
        }
      }
    `,
    { location: locationId },
  );

  const location = data?.location;
  const sortedSessions = [...location.sessions]
    .filter(
      (session): session is NonNullable<typeof session> => session != null,
    )
    .sort(
      (a, b) => (b.lastContact ?? -Infinity) - (a.lastContact ?? -Infinity),
    );

  return (
    <>
      <p>
        Use this page to create and manage access to the system through the
        kiosk module. Once a kiosk setup code has been entered into a computer,
        that computer will have access until the entry here is deleted or it
        expires. Kiosks expire if the computer using it does not access the
        system for a period of 2 weeks.
      </p>
      <p>
        An expired kiosk that was set up by scanning a QR code can be brought
        back with the <strong>Reactivate</strong> button: switch the kiosk on,
        wait for it to show its QR code screen, then reactivate it here. A kiosk
        set up with a setup code has to be set up again from scratch.
      </p>
      <p>
        <img src={bulletGreen} alt="" className="inline-block align-middle" />{" "}
        OK{" "}
        <img src={bulletOrange} alt="" className="inline-block align-middle" />{" "}
        Warning{" "}
        <img src={bulletRed} alt="" className="inline-block align-middle" />{" "}
        Problem{" "}
        <img src={bulletGray} alt="" className="inline-block align-middle" />{" "}
        Expired/Unused
      </p>
      <AdminTable>
        <thead>
          <tr>
            <Th style={{ width: 20 }}></Th>
            {isDev && <Th>ID</Th>}
            <Th>Name</Th>
            <Th>Last contact</Th>
            <Th>Code</Th>
            <Th>Device key</Th>
            <Th>Version</Th>
            <Th></Th>
          </tr>
        </thead>
        <tbody>
          {sortedSessions.map((session, idx) => (
            <Row session={session} idx={idx} key={session.id} isDev={isDev} />
          ))}
        </tbody>
      </AdminTable>
    </>
  );
}

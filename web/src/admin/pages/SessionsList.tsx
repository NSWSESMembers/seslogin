import { useState } from "react";
import { formatFullDateTime, formatSeconds } from "../../lib/time";
import { graphql, useMutation } from "react-relay";
import SessionStatus from "../components/SessionStatus";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
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

type Session = SessionsListQuery$data["location"]["sessions"][number];

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
  const { notifyError, notifySuccess } = useNotify();
  // Its computer was set up again by scanning its QR code, which moved the key onto a new
  // entry. Nothing here can revive this one — it's a leftover kept so the change is
  // visible. Needs the server's marker: a code-enrolled kiosk whose setup code has been
  // used up also ends up with no code and no key, and that one is perfectly healthy.
  const isReplacedKiosk = session.keyReleasedAt != null;
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
      isReplacedKiosk
        ? `Are you sure you want to remove this kiosk from the list? It has already been replaced and no longer works. This action cannot be undone.`
        : `Are you sure you want to delete this kiosk? Any computer using it will no longer be able to be used to access the system. This action cannot be undone.`,
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
    !isReplacedKiosk &&
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
    <tr
      className={`${idx % 2 === 0 ? "bg-surface-raised" : ""} ${
        isReplacedKiosk ? "text-ink-muted" : ""
      }`}
    >
      <Td center>
        {isReplacedKiosk ? (
          // Its last contact may be minutes old, so the live status dot would read as a
          // healthy kiosk. It isn't one any more.
          <img
            src={bulletGray}
            alt=""
            className="inline-block align-middle"
            title="Replaced"
          />
        ) : (
          <SessionStatus lastContact={session.lastContact} />
        )}
      </Td>
      {isDev && <Td className="font-mono text-[0.85em]">{session.id}</Td>}
      <Td>
        {session.name}
        {isReplacedKiosk && (
          <span
            className="ml-2 rounded-sm border border-current px-1 py-px text-[0.7em] uppercase"
            title={`This computer was set up again as a different kiosk on ${formatFullDateTime(
              new Date(session.keyReleasedAt! * 1000),
            )}, so this entry no longer works. Delete it once you don't need the record.`}
          >
            Replaced
          </span>
        )}
      </Td>
      <Td>{timeSinceAccess}</Td>
      <Td>{session.code}</Td>
      <Td>{clientVersion}</Td>
      <Td options>
        <div className="flex items-center justify-end gap-1">
          {session.reactivatable ? (
            <Button
              size="row"
              onClick={reactivateSession}
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
          {!isReplacedKiosk && (
            <ButtonLink size="row" to={`/admin/sessions/${session.id}`}>
              Edit
            </ButtonLink>
          )}
          <Button
            size="row"
            variant="danger"
            onClick={deleteSession}
            disabled={isMutationInFlight}
          >
            Delete
          </Button>
        </div>
      </Td>
    </tr>
  );
}

export default function SessionsList() {
  const { isDev } = useUserInfo();
  const selectedLocation = useSelectedLocation();
  const locationId = selectedLocation.id;
  const data = useRetryableLazyLoadQuery<SessionsListQuery>(
    graphql`
      query SessionsListQuery($location: ID!) @throwOnFieldError {
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
            reactivatable
            keyReleasedAt
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
        A kiosk marked <strong>Replaced</strong> is one whose computer has since
        been set up again by scanning its QR code, which moved it onto a
        different kiosk entry — possibly at another location. The entry stops
        working at that point and stays here only so the change is visible;
        delete it once you don't need the record.
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

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
  const [commitMutation, isMutationInFlight] =
    useMutation<SessionsListDeleteMutation>(graphql`
      mutation SessionsListDeleteMutation($id: ID!) {
        deleteSession(id: $id)
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
    <tr className={idx % 2 === 0 ? "bg-neutral-50" : undefined}>
      <Td center>
        <SessionStatus lastContact={session.lastContact} />
      </Td>
      {isDev && <Td className="font-mono text-[0.85em]">{session.id}</Td>}
      <Td>{session.name}</Td>
      <Td>{timeSinceAccess}</Td>
      <Td>{session.code}</Td>
      <Td>{clientVersion}</Td>
      <Td options>
        <div className="flex justify-end gap-1">
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
      </Td>
    </tr>
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

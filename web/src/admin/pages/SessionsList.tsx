import { useState } from "react";
import { Link } from "react-router";
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
    <>
      <tr className={idx % 2 === 0 ? "odd" : "even"}>
        <td className="center" data-label="Status">
          <SessionStatus lastContact={session.lastContact} />
        </td>
        {isDev && (
          <td style={{ fontFamily: "monospace", fontSize: "0.85em" }} data-label="ID">
            {session.id}
          </td>
        )}
        <td data-label="Name">{session.name}</td>
        <td data-label="Last Contact">{timeSinceAccess}</td>
        <td data-label="Code">{session.code}</td>
        <td data-label="Version">{clientVersion}</td>
        <td className="options" data-label="Actions">
          <Link to={`/admin/sessions/${session.id}`}>Edit</Link>&nbsp;
          <button
            className="delete"
            onClick={deleteSession}
            disabled={isMutationInFlight}
          >
            Delete
          </button>
        </td>
      </tr>
    </>
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
      <p className="icons">
        <img src={bulletGreen} alt="" /> OK <img src={bulletOrange} alt="" />{" "}
        Warning <img src={bulletRed} alt="" /> Problem{" "}
        <img src={bulletGray} alt="" /> Expired/Unused
      </p>
      <table className="admin">
        <thead>
          <tr>
            <th style={{ width: 20 }}></th>
            {isDev && <th>ID</th>}
            <th>Name</th>
            <th>Last contact</th>
            <th>Code</th>
            <th>Version</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {sortedSessions.map((session, idx) => (
            <Row session={session} idx={idx} key={session.id} isDev={isDev} />
          ))}
        </tbody>
      </table>
    </>
  );
}

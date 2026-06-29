import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import type {
  MembersListQuery,
  MembersListQuery$data,
} from "./__generated__/MembersListQuery.graphql";
import { Link } from "react-router";
import type { MembersListDeleteMutation } from "./__generated__/MembersListDeleteMutation.graphql";
import type { MembersListSyncMutation } from "./__generated__/MembersListSyncMutation.graphql";
import useSelectedLocation from "../components/useSelectedLocation";
import { formatFullDateTime } from "../../lib/time";
import bulletGreen from "../../assets/bullet-green.svg";
import { useState } from "react";
import { useUserInfo } from "../components/useUserInfo";
import { useNotify } from "../components/useNotify";

type Person = MembersListQuery$data["location"]["people"][number];

function Row({
  person,
  idx,
  isDev,
}: {
  person: Person;
  idx: number;
  isDev: boolean;
}) {
  const { notifyError, notifySuccess } = useNotify();
  const [commitMutation, isMutationInFlight] =
    useMutation<MembersListDeleteMutation>(graphql`
      mutation MembersListDeleteMutation($id: ID!) {
        deletePerson(id: $id)
      }
    `);

  async function deletePerson(event: React.MouseEvent) {
    event.preventDefault();
    const yes = confirm(
      `Are you sure you want to delete member ${person.firstName} ${person.lastName}? ` +
        "This action cannot be undone.",
    );
    if (yes) {
      try {
        await new Promise((resolve, reject) => {
          commitMutation({
            variables: { id: person.id },
            onCompleted: resolve,
            onError: reject,
            updater: (store) => {
              store.delete(person.id);
            },
          });
        });
        notifySuccess(`Member ${person.firstName} ${person.lastName} deleted`);
      } catch (err) {
        notifyError(
          err,
          `Couldn't delete member ${person.firstName} ${person.lastName}`,
        );
      }
    }
  }

  const sesApiPersonId = person.sesApiPersonId;

  return (
    <tr className={idx % 2 === 0 ? "odd" : "even"}>
      <td className="center" data-label="Status">
        {sesApiPersonId ? (
          <img
            src={bulletGreen}
            alt=""
            title={sesApiPersonId}
            width={12}
            height={12}
            style={{ verticalAlign: "middle" }}
          />
        ) : null}
      </td>
      {isDev && (
        <td
          style={{ fontFamily: "monospace", fontSize: "0.85em" }}
          data-label="ID"
        >
          {person.id}
        </td>
      )}
      <td data-label="SES ID">{person.memberNumber}</td>
      <td className="nowrap" data-label="Name">
        {person.firstName} {person.lastName}
      </td>
      <td className="options" data-label="Actions">
        <Link to={`/admin/members/activity/${person.id}`}>Activity</Link>
        {!sesApiPersonId ? (
          <>
            &nbsp;
            <Link to={`/admin/members/${person.id}`}>Edit</Link>&nbsp;
            <button
              className="delete"
              onClick={deletePerson}
              disabled={isMutationInFlight}
            >
              Delete
            </button>
          </>
        ) : null}
      </td>
    </tr>
  );
}

export default function MembersList() {
  const { isDev } = useUserInfo();
  const selectedLocation = useSelectedLocation();
  const locationId = selectedLocation.id;
  const data = useLazyLoadQuery<MembersListQuery>(
    graphql`
      query MembersListQuery($location: ID!) {
        location(id: $location) {
          id
          sesApiHeadquartersId
          lastSuccessfulMemberSync
          people {
            id
            firstName
            lastName
            memberNumber
            sesApiPersonId
          }
        }
      }
    `,
    { location: locationId },
    { fetchKey: locationId },
  );

  const [commitSync, isSyncInFlight] = useMutation<MembersListSyncMutation>(
    graphql`
      mutation MembersListSyncMutation($locationId: ID!) {
        enqueueMemberSync(locationId: $locationId)
      }
    `,
  );
  const { notifyError, notifySuccess } = useNotify();

  function triggerSync() {
    commitSync({
      variables: { locationId },
      onCompleted: () => {
        notifySuccess("Member sync queued");
      },
      onError: (err) => {
        notifyError(err, "Couldn't queue member sync");
      },
    });
  }

  const location = data?.location;
  const sortedPeople = [...location.people]
    .filter((person): person is NonNullable<typeof person> => person != null)
    .sort((a, b) =>
      `${a.firstName} ${a.lastName}`.localeCompare(
        `${b.firstName} ${b.lastName}`,
      ),
    );

  const lastSync = location.lastSuccessfulMemberSync;
  const lastSyncText = lastSync
    ? formatFullDateTime(new Date(lastSync * 1000))
    : "Never";
  const [now] = useState(() => Date.now() / 1000);
  const syncedRecently = lastSync != null && now - lastSync < 3600;

  return (
    <>
      {location.sesApiHeadquartersId ? (
        <div style={{ marginBottom: 8 }}>
          Last successful member sync: {lastSyncText}&nbsp;&nbsp;
          {!syncedRecently && (
            <button onClick={triggerSync} disabled={isSyncInFlight}>
              Sync now
            </button>
          )}
        </div>
      ) : null}
      <table className="admin">
        <thead>
          <tr>
            <th style={{ width: 20 }}></th>
            {isDev && <th>ID</th>}
            <th style={{ width: 100 }}>SES ID</th>
            <th>Name</th>
            <th style={{ width: 100 }}></th>
          </tr>
        </thead>
        <tbody>
          {sortedPeople.map((person, idx) => (
            <Row key={person.id} person={person} idx={idx} isDev={isDev} />
          ))}
        </tbody>
      </table>
    </>
  );
}

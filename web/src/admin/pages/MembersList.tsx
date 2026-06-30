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
  const badgeCount = person.badges.length;
  const lastBadgeEarnedAt =
    badgeCount > 0
      ? [...person.badges].sort((a, b) => b.awardedAt - a.awardedAt)[0]
          .awardedAt
      : null;

  return (
    <tr className={idx % 2 === 0 ? "odd" : "even"}>
      <td className="center">
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
        <td style={{ fontFamily: "monospace", fontSize: "0.85em" }}>
          {person.id}
        </td>
      )}
      <td>{person.memberNumber}</td>
      <td className="nowrap">
        {person.firstName} {person.lastName}
      </td>
      <td className="nowrap">
        {badgeCount > 0 ? (
          <>
            <span className="member-badge-count">{badgeCount}</span>
            <span className="member-badge-last-earned">
              {lastBadgeEarnedAt
                ? formatFullDateTime(new Date(lastBadgeEarnedAt * 1000))
                : ""}
            </span>
          </>
        ) : (
          <span className="member-badge-last-earned">None</span>
        )}
      </td>
      <td className="options">
        <Link to={`/admin/members/activity/${person.id}`}>Activity</Link>
        &nbsp;
        <Link to={`/admin/members/badges/${person.id}`}>Badges</Link>
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
            badges(locationId: $location) {
              id
              awardedAt
            }
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
            <th style={{ width: 210 }}>Badges</th>
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

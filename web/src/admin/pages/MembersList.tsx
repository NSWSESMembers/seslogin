import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import type {
  MembersListQuery,
  MembersListQuery$data,
} from "./__generated__/MembersListQuery.graphql";
import type { MembersListDeleteMutation } from "./__generated__/MembersListDeleteMutation.graphql";
import type { MembersListSyncMutation } from "./__generated__/MembersListSyncMutation.graphql";
import useSelectedLocation from "../components/useSelectedLocation";
import { formatFullDateTime } from "../../lib/time";
import bulletGreen from "../../assets/bullet-green.svg";
import { useState } from "react";
import { useUserInfo } from "../components/useUserInfo";
import { useNotify } from "../components/useNotify";
import { AdminTable, Th, Td } from "../../components/ui/Table";
import { Button, ButtonLink } from "../../components/ui/Button";

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
    <tr className={idx % 2 === 0 ? "bg-neutral-50" : undefined}>
      <Td center>
        {sesApiPersonId ? (
          <img
            src={bulletGreen}
            alt=""
            title={sesApiPersonId}
            width={12}
            height={12}
            className="max-w-none align-middle"
          />
        ) : null}
      </Td>
      {isDev && <Td className="font-mono text-[0.85em]">{person.id}</Td>}
      <Td>{person.memberNumber}</Td>
      <Td nowrap>
        {person.firstName} {person.lastName}
      </Td>
      <Td options>
        <div className="flex justify-end gap-1">
          <ButtonLink size="row" to={`/admin/members/activity/${person.id}`}>
            Activity
          </ButtonLink>
          {!sesApiPersonId ? (
            <>
              <ButtonLink size="row" to={`/admin/members/${person.id}`}>
                Edit
              </ButtonLink>
              <Button
                size="row"
                variant="danger"
                onClick={deletePerson}
                disabled={isMutationInFlight}
              >
                Delete
              </Button>
            </>
          ) : null}
        </div>
      </Td>
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
        <div className="mb-2">
          Last successful member sync: {lastSyncText}{" "}
          {!syncedRecently && (
            <button onClick={triggerSync} disabled={isSyncInFlight}>
              Sync now
            </button>
          )}
        </div>
      ) : null}
      <AdminTable>
        <thead>
          <tr>
            <Th style={{ width: 20 }}></Th>
            {isDev && <Th>ID</Th>}
            <Th style={{ width: 100 }}>SES ID</Th>
            <Th>Name</Th>
            <Th style={{ width: 100 }}></Th>
          </tr>
        </thead>
        <tbody>
          {sortedPeople.map((person, idx) => (
            <Row key={person.id} person={person} idx={idx} isDev={isDev} />
          ))}
        </tbody>
      </AdminTable>
    </>
  );
}

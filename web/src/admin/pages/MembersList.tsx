import { graphql, useMutation } from "react-relay";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import type {
  MembersListQuery,
  MembersListQuery$data,
} from "./__generated__/MembersListQuery.graphql";
import type { MembersListDeleteMutation } from "./__generated__/MembersListDeleteMutation.graphql";
import type { MembersListSyncMutation } from "./__generated__/MembersListSyncMutation.graphql";
import useSelectedLocation from "../components/useSelectedLocation";
import { formatFullDateTime } from "../../lib/time";
import bulletGreen from "../../assets/bullet-green.svg";
import bulletOrange from "../../assets/bullet-orange.svg";
import { useRef, useState } from "react";
import { useUserInfo } from "../components/useUserInfo";
import { useNotify } from "../components/useNotify";
import { AdminTable, Th, Td } from "../../components/ui/Table";
import { Button, ButtonLink } from "../../components/ui/Button";
import { Popover } from "../../components/ui/Popover";

type Person = MembersListQuery$data["location"]["people"][number];

// The status bullet as a trigger: its explanation is on the `title` (hover) and in a
// popover on click, so it also works on touch — same pattern as CommentIndicator.
function StatusBullet({
  src,
  label,
  detail,
}: {
  src: string;
  label: string;
  detail: string;
}) {
  const [open, setOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);

  return (
    <>
      <button
        ref={buttonRef}
        type="button"
        title={detail}
        aria-label={open ? `Hide ${label}` : `Show ${label}`}
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="cursor-help align-middle"
      >
        <img
          src={src}
          alt=""
          width={12}
          height={12}
          className="max-w-none align-middle"
        />
      </button>
      {open && (
        <Popover
          anchorRef={buttonRef}
          onDismiss={() => setOpen(false)}
          className="max-w-xs px-2 py-1.5 text-sm"
        >
          {detail}
        </Popover>
      )}
    </>
  );
}

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
  // Member sync has stopped seeing this person in SES. They are soft-deleted once the
  // marker ages past the grace window, unless a later sync finds them again.
  const missingSince = person.missingSince;

  return (
    <tr className={idx % 2 === 0 ? "bg-surface-raised" : undefined}>
      <Td center>
        {missingSince ? (
          <StatusBullet
            src={bulletOrange}
            label="sync status"
            detail={`Missing from SES since ${formatFullDateTime(
              new Date(missingSince * 1000),
            )} — pending deletion`}
          />
        ) : sesApiPersonId ? (
          <StatusBullet
            src={bulletGreen}
            label="SES ID"
            detail={sesApiPersonId}
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
  const data = useRetryableLazyLoadQuery<MembersListQuery>(
    graphql`
      query MembersListQuery($location: ID!) @throwOnFieldError {
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
            missingSince
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
      <p className="my-4">
        This list shows only members who belong to this unit.
      </p>
      {location.sesApiHeadquartersId ? (
        <div className="mb-2">
          Last successful member sync: {lastSyncText}{" "}
          {!syncedRecently && (
            <button
              className="cursor-pointer"
              onClick={triggerSync}
              disabled={isSyncInFlight}
            >
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

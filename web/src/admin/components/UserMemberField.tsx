import { Suspense, useState } from "react";
import { graphql, useFragment, useMutation } from "react-relay";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import type { UserMemberField_user$key } from "./__generated__/UserMemberField_user.graphql";
import type { UserMemberFieldMutation } from "./__generated__/UserMemberFieldMutation.graphql";
import type { UserMemberFieldPeopleQuery } from "./__generated__/UserMemberFieldPeopleQuery.graphql";
import { useNotify } from "./useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import Combobox from "../../components/ui/Combobox";
import { Button } from "../../components/ui/Button";
import LoadingIndicator from "../../components/LoadingIndicator";
import RelayErrorBoundary from "../../components/RelayErrorBoundary";

interface Props {
  user: UserMemberField_user$key;
  locations: readonly { id: string; name: string }[];
}

/**
 * Sets the member a user acts as when signing in or out from the mobile app.
 * Pick a unit, then a member from it — both typeahead — since members are only
 * listed per unit.
 */
export default function UserMemberField({ user, locations }: Props) {
  const { id: userId, person: member } = useFragment(
    graphql`
      fragment UserMemberField_user on User {
        id
        person {
          id
          firstName
          lastName
          memberNumber
          location {
            id
            name
          }
        }
      }
    `,
    user,
  );
  const { notifyError, notifySuccess } = useNotify();
  const [unitId, setUnitId] = useState(member?.location.id ?? "");
  const [personId, setPersonId] = useState("");

  const [commit, isInFlight] = useMutation<UserMemberFieldMutation>(graphql`
    mutation UserMemberFieldMutation($userId: ID!, $personId: ID) {
      setUserMember(userId: $userId, personId: $personId) {
        ...UserMemberField_user
      }
    }
  `);

  function save(newPersonId: string | null) {
    commit({
      variables: { userId, personId: newPersonId },
      onCompleted: () => {
        setPersonId("");
        notifySuccess(newPersonId ? "Member set" : "Member cleared");
      },
      onError: (err) =>
        notifyError(
          err,
          newPersonId ? "Couldn't set member" : "Couldn't clear member",
        ),
    });
  }

  return (
    <>
      <p className="my-4">
        The member this user signs in and out as from the SESLogin mobile app.
      </p>
      <FieldList>
        <FormField label="Current member">
          {member ? (
            <span>
              {member.firstName} {member.lastName}
              {member.memberNumber ? ` (${member.memberNumber})` : ""},{" "}
              {member.location.name}
            </span>
          ) : (
            <span>None</span>
          )}
        </FormField>
        <FormField label={<label htmlFor="member-unit">Unit</label>}>
          <Combobox
            id="member-unit"
            placeholder="-- Select unit --"
            emptyText="No units match"
            value={unitId}
            onChange={(value) => {
              setUnitId(value);
              setPersonId("");
            }}
            options={locations.map((l) => ({ value: l.id, label: l.name }))}
          />
        </FormField>
        {unitId && (
          <FormField label={<label htmlFor="member-person">Member</label>}>
            <RelayErrorBoundary resetKey={unitId} canRetry>
              <Suspense fallback={<LoadingIndicator />}>
                <UnitMemberCombobox
                  locationId={unitId}
                  value={personId}
                  onChange={setPersonId}
                />
              </Suspense>
            </RelayErrorBoundary>
          </FormField>
        )}
        <FormField>
          <div className="flex gap-2">
            <Button
              type="button"
              disabled={isInFlight || !personId || personId === member?.id}
              onClick={() => save(personId)}
            >
              Set member
            </Button>
            {member && (
              <Button
                type="button"
                variant="danger"
                disabled={isInFlight}
                onClick={() => save(null)}
              >
                Clear member
              </Button>
            )}
          </div>
        </FormField>
      </FieldList>
    </>
  );
}

function UnitMemberCombobox({
  locationId,
  value,
  onChange,
}: {
  locationId: string;
  value: string;
  onChange: (value: string) => void;
}) {
  const data = useRetryableLazyLoadQuery<UserMemberFieldPeopleQuery>(
    graphql`
      query UserMemberFieldPeopleQuery($location: ID!) @throwOnFieldError {
        location(id: $location) {
          people {
            id
            firstName
            lastName
            memberNumber
          }
        }
      }
    `,
    { location: locationId },
  );
  const people = [...data.location.people].sort((a, b) =>
    `${a.firstName} ${a.lastName}`.localeCompare(
      `${b.firstName} ${b.lastName}`,
    ),
  );

  return (
    <Combobox
      id="member-person"
      placeholder="-- Select member --"
      emptyText="No members match"
      value={value}
      onChange={onChange}
      options={people.map((person) => ({
        value: person.id,
        label: `${person.firstName} ${person.lastName}`,
        // Searchable by member number as well as by name.
        description: person.memberNumber ?? undefined,
      }))}
    />
  );
}

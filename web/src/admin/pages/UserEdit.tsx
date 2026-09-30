import { useState } from "react";
import { useNavigate, useParams } from "react-router";
import { graphql, useMutation } from "react-relay";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import type { UserEditQuery } from "./__generated__/UserEditQuery.graphql";
import type { UserEditMutation } from "./__generated__/UserEditMutation.graphql";
import type { UserEditRevokeOauthGrantMutation } from "./__generated__/UserEditRevokeOauthGrantMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import TextInput from "../../components/ui/TextInput";
import { Button } from "../../components/ui/Button";
import LocationRolesField from "../components/LocationRolesField";
import {
  joinLocationRoles,
  splitLocationRoles,
  type LocationRoleEntry,
} from "../components/locationRoles";
import { AdminTable, Th, Td } from "../../components/ui/Table";
import { SectionHeading } from "../../components/ui/SectionHeading";
import { formatFullDateTime } from "../../lib/time";

export default function UserEdit() {
  const navigate = useNavigate();
  const params = useParams();
  const { notifyError, notifySuccess } = useNotify();
  const id = params.userId!;

  const data = useRetryableLazyLoadQuery<UserEditQuery>(
    graphql`
      query UserEditQuery($id: ID!) @throwOnFieldError {
        user(id: $id) {
          id
          email
          isSuper
          isDev
          enabled
          locationGrantIds
          readOnlyLocationGrantIds
          oauthGrants {
            id
            clientName
            redirectHost
            createdAt
            lastUsedAt
          }
        }
        locations {
          id
          name
        }
      }
    `,
    { id },
  );

  const [commitMutation, isMutationInFlight] = useMutation<UserEditMutation>(
    graphql`
      mutation UserEditMutation(
        $id: ID!
        $email: String!
        $isSuper: Boolean!
        $isDev: Boolean!
        $locationGrants: [String!]!
        $readOnlyLocationGrants: [String!]!
        $enabled: Boolean!
      ) {
        updateUser(
          id: $id
          email: $email
          isSuper: $isSuper
          isDev: $isDev
          locationGrants: $locationGrants
          readOnlyLocationGrants: $readOnlyLocationGrants
          enabled: $enabled
        ) {
          id
          email
          isSuper
          isDev
          locationGrantIds
          readOnlyLocationGrantIds
        }
      }
    `,
  );

  async function handleSubmit(formData: FormData) {
    const email = formData.get("email")?.toString() || "";
    const isSuper = formData.get("super") === "on";
    const isDev = formData.get("dev") === "on";
    const enabled = formData.get("enabled") === "on";
    // A super user has access everywhere, so the role editor is hidden and
    // both grant lists are cleared.
    const { locationGrants, readOnlyLocationGrants } = isSuper
      ? { locationGrants: [], readOnlyLocationGrants: [] }
      : splitLocationRoles(roles);

    try {
      await new Promise((resolve, reject) => {
        commitMutation({
          variables: {
            id,
            email,
            isSuper,
            isDev,
            locationGrants,
            readOnlyLocationGrants,
            enabled,
          },
          onCompleted: resolve,
          onError: reject,
          updater: (store) => {
            store.invalidateStore();
          },
        });
      });
    } catch (err) {
      notifyError(err, "Couldn't save user");
      return;
    }

    notifySuccess("User saved");
    navigate("/admin/users");
  }

  const [commitRevoke, isRevokeInFlight] =
    useMutation<UserEditRevokeOauthGrantMutation>(graphql`
      mutation UserEditRevokeOauthGrantMutation($id: ID!) {
        revokeOauthGrant(id: $id)
      }
    `);

  function handleRevoke(grantId: string, clientName: string) {
    if (
      !window.confirm(
        `Disconnect "${clientName}"? It will need to be reconnected to act as this user again.`,
      )
    ) {
      return;
    }
    commitRevoke({
      variables: { id: grantId },
      onCompleted: () => notifySuccess(`Disconnected "${clientName}"`),
      onError: (err) => notifyError(err, `Couldn't disconnect "${clientName}"`),
      updater: (store) => {
        store.invalidateStore();
      },
    });
  }

  const locations = [...data.locations].sort((a, b) =>
    a.name.localeCompare(b.name),
  );
  const user = data.user;
  const [isSuper, setIsSuper] = useState(user.isSuper);
  const [isDev, setIsDev] = useState(user.isDev);
  const [enabled, setEnabled] = useState(user.enabled);
  const [roles, setRoles] = useState<LocationRoleEntry[]>(() =>
    joinLocationRoles(user.locationGrantIds, user.readOnlyLocationGrantIds),
  );

  return (
    <>
      <p className="my-4">Edit the member's details, then click Save.</p>

      <form action={handleSubmit}>
        <FieldList>
          <FormField label={<label htmlFor="email">Email</label>}>
            <TextInput
              type="email"
              name="email"
              id="email"
              defaultValue={user.email}
              required
            />
          </FormField>
          <FormField label={<label htmlFor="super">Super</label>}>
            <input
              type="checkbox"
              name="super"
              id="super"
              checked={isSuper}
              onChange={(e) => setIsSuper(e.target.checked)}
            />
          </FormField>
          <FormField label={<label htmlFor="dev">Dev</label>}>
            <input
              type="checkbox"
              name="dev"
              id="dev"
              checked={isDev}
              onChange={(e) => setIsDev(e.target.checked)}
            />
          </FormField>
          <FormField label={<label htmlFor="enabled">Enabled</label>}>
            <input
              type="checkbox"
              name="enabled"
              id="enabled"
              checked={enabled}
              onChange={(e) => setEnabled(e.target.checked)}
            />
          </FormField>
          {!isSuper && (
            <FormField label={<label htmlFor="locations">Locations</label>}>
              <LocationRolesField
                id="locations"
                locations={locations.map((l) => ({ id: l.id, name: l.name }))}
                value={roles}
                onChange={setRoles}
              />
            </FormField>
          )}
          <FormField>
            <Button type="submit" disabled={isMutationInFlight}>
              Save
            </Button>
          </FormField>
        </FieldList>
      </form>

      <SectionHeading>Connected apps</SectionHeading>
      {user.oauthGrants.length === 0 && (
        <p className="my-4">No apps connected.</p>
      )}
      {user.oauthGrants.length > 0 && (
        <AdminTable>
          <thead>
            <tr>
              <Th>App</Th>
              <Th>Redirects to</Th>
              <Th>Connected</Th>
              <Th>Last used</Th>
              <Th></Th>
            </tr>
          </thead>
          <tbody>
            {user.oauthGrants.map((grant, idx) => (
              <tr
                key={grant.id}
                className={idx % 2 === 0 ? "bg-surface-raised" : undefined}
              >
                <Td>{grant.clientName}</Td>
                <Td>{grant.redirectHost}</Td>
                <Td>{formatFullDateTime(new Date(grant.createdAt * 1000))}</Td>
                <Td>
                  {grant.lastUsedAt
                    ? formatFullDateTime(new Date(grant.lastUsedAt * 1000))
                    : "Never"}
                </Td>
                <Td options>
                  <div className="flex justify-end gap-1">
                    <Button
                      size="row"
                      variant="danger"
                      disabled={isRevokeInFlight}
                      onClick={() => handleRevoke(grant.id, grant.clientName)}
                    >
                      Disconnect
                    </Button>
                  </div>
                </Td>
              </tr>
            ))}
          </tbody>
        </AdminTable>
      )}
    </>
  );
}

import { useState } from "react";
import {
  graphql,
  useFragment,
  useLazyLoadQuery,
  useMutation,
} from "react-relay";
import type { UserListQuery } from "./__generated__/UserListQuery.graphql";
import type { UserListToggleMutation } from "./__generated__/UserListToggleMutation.graphql";
import type { UserList_user$key } from "./__generated__/UserList_user.graphql";
import { formatTimeDiff } from "../../lib/time";
import { useUserInfo } from "../components/useUserInfo";
import { useNotify } from "../components/useNotify";
import { AdminTable, Th, Td } from "../../components/ui/Table";
import { Button, ButtonLink } from "../../components/ui/Button";

function Row(props: { user: UserList_user$key; idx: number; isDev: boolean }) {
  const isDev = props.isDev;
  const { notifyError, notifySuccess } = useNotify();
  const user = useFragment<UserList_user$key>(
    graphql`
      fragment UserList_user on User {
        id
        email
        accessTime
        isSuper
        isDev
        locationGrantIds
        locations {
          id
          name
        }
        enabled
      }
    `,
    props.user,
  );

  const [commitMutation, isMutationInFlight] =
    useMutation<UserListToggleMutation>(graphql`
      mutation UserListToggleMutation(
        $id: ID!
        $email: String!
        $isSuper: Boolean!
        $isDev: Boolean!
        $locationGrants: [String!]!
        $enabled: Boolean!
      ) {
        updateUser(
          id: $id
          email: $email
          isSuper: $isSuper
          isDev: $isDev
          locationGrants: $locationGrants
          enabled: $enabled
        ) {
          id
          enabled
        }
      }
    `);

  async function toggleEnabled() {
    const action = user.enabled ? "disable" : "enable";
    const yes = confirm(
      `Are you sure you want to ${action} user ${user.email}?`,
    );
    if (yes) {
      try {
        await new Promise((resolve, reject) => {
          commitMutation({
            variables: {
              id: user.id,
              email: user.email,
              isSuper: user.isSuper,
              isDev: user.isDev,
              locationGrants: [...user.locationGrantIds],
              enabled: !user.enabled,
            },
            onCompleted: resolve,
            onError: reject,
            updater: (store) => {
              store.invalidateStore();
            },
          });
        });
        notifySuccess(`User ${user.email} ${action}d`);
      } catch (err) {
        notifyError(err, `Couldn't ${action} user ${user.email}`);
      }
    }
  }

  return (
    <tr className={props.idx % 2 === 0 ? "bg-neutral-50" : undefined}>
      {isDev && <Td className="font-mono text-[0.85em]">{user.id}</Td>}
      <Td>
        <span className={user.enabled ? undefined : "line-through"}>
          {user.email}
        </span>
      </Td>
      <Td>
        {user.accessTime
          ? formatTimeDiff(new Date(user.accessTime * 1000), new Date()) +
            " ago"
          : "-"}
      </Td>
      <Td>{user.isSuper ? "Yes" : "No"}</Td>
      <Td>
        {user.isSuper ? null : user.locations.map((l) => l.name).join(", ")}
      </Td>
      <Td options>
        <div className="flex justify-end gap-1">
          <ButtonLink size="row" to={`/admin/users/${user.id}`}>
            Edit
          </ButtonLink>
          <Button
            size="row"
            variant={user.enabled ? "danger" : "primary"}
            onClick={toggleEnabled}
            disabled={isMutationInFlight}
          >
            {user.enabled ? "Disable" : "Enable"}
          </Button>
        </div>
      </Td>
    </tr>
  );
}

export default function UserList() {
  const { isDev } = useUserInfo();
  const [showDisabled, setShowDisabled] = useState(false);
  const data = useLazyLoadQuery<UserListQuery>(
    graphql`
      query UserListQuery {
        users {
          id
          accessTime
          enabled
          ...UserList_user
        }
      }
    `,
    {},
  );

  const users = [...(data?.users || [])]
    .filter((u) => showDisabled || u.enabled)
    .sort((a, b) => {
      const aAccessTime = a.accessTime ?? -Infinity;
      const bAccessTime = b.accessTime ?? -Infinity;
      return bAccessTime - aAccessTime;
    });

  return (
    <>
      <p>
        <label>
          <input
            type="checkbox"
            checked={showDisabled}
            onChange={(e) => setShowDisabled(e.target.checked)}
          />{" "}
          Show disabled
        </label>
      </p>
      <AdminTable>
        <thead>
          <tr>
            {isDev && <Th>ID</Th>}
            <Th>Email</Th>
            <Th>Last Access</Th>
            <Th>Super</Th>
            <Th>Locations</Th>
            <Th style={{ width: 100 }}></Th>
          </tr>
        </thead>
        <tbody>
          {users.map((user, idx) => (
            <Row key={user.id} user={user} idx={idx} isDev={isDev} />
          ))}
        </tbody>
      </AdminTable>
    </>
  );
}

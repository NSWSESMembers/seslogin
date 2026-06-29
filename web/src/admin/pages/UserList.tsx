import { useState } from "react";
import { Link } from "react-router";
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
    <tr className={props.idx % 2 === 0 ? "odd" : "even"}>
      {isDev && (
        <td
          style={{ fontFamily: "monospace", fontSize: "0.85em" }}
          data-label="ID"
        >
          {user.id}
        </td>
      )}
      <td data-label="Email">
        <span className={user.enabled ? "" : "strike"}>{user.email}</span>
      </td>
      <td data-label="Last Access">
        {user.accessTime
          ? formatTimeDiff(new Date(user.accessTime * 1000), new Date()) +
            " ago"
          : "-"}
      </td>
      <td data-label="Super">{user.isSuper ? "Yes" : "No"}</td>
      <td data-label="Locations">
        {user.isSuper ? null : user.locations.map((l) => l.name).join(", ")}
      </td>
      <td className="options" data-label="Actions">
        <Link to={`/admin/users/${user.id}`}>Edit</Link>&nbsp;
        <button
          className={user.enabled ? "delete" : ""}
          onClick={toggleEnabled}
          disabled={isMutationInFlight}
        >
          {user.enabled ? "Disable" : "Enable"}
        </button>
      </td>
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
      <table className="admin">
        <thead>
          <tr>
            {isDev && <th>ID</th>}
            <th>Email</th>
            <th>Last Access</th>
            <th>Super</th>
            <th>Locations</th>
            <th style={{ width: 100 }}></th>
          </tr>
        </thead>
        <tbody>
          {users.map((user, idx) => (
            <Row key={user.id} user={user} idx={idx} isDev={isDev} />
          ))}
        </tbody>
      </table>
    </>
  );
}

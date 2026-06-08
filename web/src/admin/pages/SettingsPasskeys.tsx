import { useState } from "react";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import type { SettingsPasskeysQuery } from "./__generated__/SettingsPasskeysQuery.graphql";
import type { SettingsPasskeysRenameMutation } from "./__generated__/SettingsPasskeysRenameMutation.graphql";
import type { SettingsPasskeysDeleteMutation } from "./__generated__/SettingsPasskeysDeleteMutation.graphql";
import { usePasskeyRegistration } from "../components/usePasskeyRegistration";
import { useNotify } from "../components/useNotify";
import { getErrorMessage } from "../../lib/relayErrors";
import {
  browserSupportsWebAuthn,
  markPasskeyEnrollPromptShown,
} from "../../lib/passkey";

const MAX_PASSKEYS = 10;

export default function SettingsPasskeys() {
  // Bumped after any add/rename/delete to force the query to re-read the
  // passkey list from the server. Invalidating the store alone doesn't
  // re-render a mounted useLazyLoadQuery, and these mutations don't return the
  // full list, so a refetch is the simplest way to keep the table in sync.
  const [refreshKey, setRefreshKey] = useState(0);
  const refresh = () => setRefreshKey((k) => k + 1);

  const data = useLazyLoadQuery<SettingsPasskeysQuery>(
    graphql`
      query SettingsPasskeysQuery {
        user {
          id
          passkeys {
            id
            name
            createdAt
            lastUsedAt
          }
        }
      }
    `,
    {},
    { fetchPolicy: "store-and-network", fetchKey: refreshKey },
  );

  const passkeys = data.user.passkeys;
  const atCap = passkeys.length >= MAX_PASSKEYS;

  const register = usePasskeyRegistration();
  const { notifyError, notifySuccess } = useNotify();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const supported = browserSupportsWebAuthn();

  const [commitRename] = useMutation<SettingsPasskeysRenameMutation>(graphql`
    mutation SettingsPasskeysRenameMutation($id: String!, $name: String!) {
      renamePasskey(id: $id, name: $name) {
        id
        name
      }
    }
  `);

  const [commitDelete] = useMutation<SettingsPasskeysDeleteMutation>(graphql`
    mutation SettingsPasskeysDeleteMutation($id: String!) {
      deletePasskey(id: $id)
    }
  `);

  async function handleAdd() {
    const name = window.prompt(
      "Name this passkey (e.g. the device you're using):",
      "My device",
    );
    if (name === null) return;
    const trimmed = name.trim();
    if (!trimmed) return;
    setBusy(true);
    setError(null);
    try {
      await register(trimmed);
      refresh();
      notifySuccess("Passkey added");
    } catch (err) {
      setError(`Couldn't add a passkey: ${getErrorMessage(err)}`);
    } finally {
      setBusy(false);
    }
  }

  function handleRename(id: string, current: string) {
    const name = window.prompt("Rename passkey:", current);
    if (name === null) return;
    const trimmed = name.trim();
    if (!trimmed) return;
    commitRename({
      variables: { id, name: trimmed },
      onCompleted: () => {
        refresh();
        notifySuccess("Passkey renamed");
      },
      onError: (err) => notifyError(err, "Couldn't rename passkey"),
    });
  }

  function handleDelete(id: string, name: string) {
    if (!window.confirm(`Delete passkey "${name}"? This cannot be undone.`)) {
      return;
    }
    // If this is the user's last passkey, arm the 12h enrollment-prompt throttle.
    // Deleting down to zero would otherwise satisfy the interstitial's "no
    // passkeys" condition and pop the "Add a passkey" nag on the next reload —
    // immediately after the user deliberately removed their passkeys.
    const wasLastPasskey = passkeys.length === 1;
    commitDelete({
      variables: { id },
      onCompleted: () => {
        if (wasLastPasskey) markPasskeyEnrollPromptShown();
        refresh();
        notifySuccess("Passkey deleted");
      },
      onError: (err) => notifyError(err, "Couldn't delete passkey"),
    });
  }

  return (
    <div className="passkeys-settings">
      <h2>Passkeys</h2>
      <p>
        Passkeys let you sign in with Face ID, Touch ID, or your device PIN
        instead of an email code. You can save up to {MAX_PASSKEYS}.
      </p>
      {!supported && (
        <p className="action-panel__message action-panel__message--warning">
          This browser or device does not support passkeys.
        </p>
      )}
      {error && (
        <p className="action-panel__message action-panel__message--error">
          {error}
        </p>
      )}
      {passkeys.length === 0 && <p>No passkeys saved yet.</p>}
      {passkeys.length > 0 && (
        <table className="admin">
          <thead>
            <tr>
              <th>Name</th>
              <th>Added</th>
              <th>Last used</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {passkeys.map((pk, idx) => (
              <tr key={pk.id} className={idx % 2 === 0 ? "odd" : "even"}>
                <td>{pk.name}</td>
                <td>{formatDate(pk.createdAt)}</td>
                <td>{pk.lastUsedAt ? formatDate(pk.lastUsedAt) : "Never"}</td>
                <td className="options">
                  <button
                    type="button"
                    onClick={() => handleRename(pk.id, pk.name)}
                  >
                    Rename
                  </button>
                  &nbsp;
                  <button
                    type="button"
                    className="delete"
                    onClick={() => handleDelete(pk.id, pk.name)}
                  >
                    Delete
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <p>
        <button
          type="button"
          className="button"
          onClick={handleAdd}
          disabled={busy || atCap || !supported}
        >
          {busy ? "Setting up…" : "Add a passkey"}
        </button>
        {atCap && (
          <span>&nbsp; You&apos;ve reached the maximum of {MAX_PASSKEYS}.</span>
        )}
      </p>
    </div>
  );
}

function formatDate(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

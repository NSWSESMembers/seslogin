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
import { PanelMessage } from "../../components/ui/Panel";
import { AdminTable, Th, Td } from "../../components/ui/Table";
import { Button } from "../../components/ui/Button";

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
    <div>
      <h2>Passkeys</h2>
      <p>
        Passkeys let you sign in with Face ID, Touch ID, or your device PIN
        instead of an email code. You can save up to {MAX_PASSKEYS}.
      </p>
      {!supported && (
        <PanelMessage variant="warning">
          This browser or device does not support passkeys.
        </PanelMessage>
      )}
      {error && <PanelMessage>{error}</PanelMessage>}
      {passkeys.length === 0 && <p>No passkeys saved yet.</p>}
      {passkeys.length > 0 && (
        <AdminTable>
          <thead>
            <tr>
              <Th>Name</Th>
              <Th>Added</Th>
              <Th>Last used</Th>
              <Th></Th>
            </tr>
          </thead>
          <tbody>
            {passkeys.map((pk, idx) => (
              <tr
                key={pk.id}
                className={idx % 2 === 0 ? "bg-neutral-50" : undefined}
              >
                <Td>{pk.name}</Td>
                <Td>{formatDate(pk.createdAt)}</Td>
                <Td>{pk.lastUsedAt ? formatDate(pk.lastUsedAt) : "Never"}</Td>
                <Td options>
                  <div className="flex justify-end gap-1">
                    <Button
                      size="row"
                      onClick={() => handleRename(pk.id, pk.name)}
                    >
                      Rename
                    </Button>
                    <Button
                      size="row"
                      variant="danger"
                      onClick={() => handleDelete(pk.id, pk.name)}
                    >
                      Delete
                    </Button>
                  </div>
                </Td>
              </tr>
            ))}
          </tbody>
        </AdminTable>
      )}
      <p>
        <Button onClick={handleAdd} disabled={busy || atCap || !supported}>
          {busy ? "Setting up…" : "Add a passkey"}
        </Button>
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

import { useState } from "react";
import { graphql, useMutation } from "react-relay";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import type { SettingsConnectedAppsQuery } from "./__generated__/SettingsConnectedAppsQuery.graphql";
import type { SettingsConnectedAppsRevokeMutation } from "./__generated__/SettingsConnectedAppsRevokeMutation.graphql";
import { formatFullDateTime } from "../../lib/time";
import { useNotify } from "../components/useNotify";
import { AdminTable, Th, Td } from "../../components/ui/Table";
import { Button } from "../../components/ui/Button";
import { SectionHeading } from "../../components/ui/SectionHeading";

export default function SettingsConnectedApps() {
  // Same reasoning as SettingsPasskeys: the mutation doesn't return the full
  // list, so a bumped fetchKey is the simplest way to re-read it after a
  // revoke.
  const [refreshKey, setRefreshKey] = useState(0);
  const refresh = () => setRefreshKey((k) => k + 1);

  const data = useRetryableLazyLoadQuery<SettingsConnectedAppsQuery>(
    graphql`
      query SettingsConnectedAppsQuery @throwOnFieldError {
        user {
          id
          oauthGrants {
            id
            clientName
            redirectHost
            createdAt
            lastUsedAt
            refreshExpiresAt
          }
        }
      }
    `,
    {},
    { fetchPolicy: "store-and-network", fetchKey: refreshKey },
  );

  const grants = data.user.oauthGrants;
  const { notifyError, notifySuccess } = useNotify();

  const [commitRevoke, isRevokeInFlight] =
    useMutation<SettingsConnectedAppsRevokeMutation>(graphql`
      mutation SettingsConnectedAppsRevokeMutation($id: ID!) {
        revokeOauthGrant(id: $id)
      }
    `);

  function handleRevoke(id: string, clientName: string) {
    if (
      !window.confirm(
        `Disconnect "${clientName}"? It will need to be reconnected to act as you again.`,
      )
    ) {
      return;
    }
    commitRevoke({
      variables: { id },
      onCompleted: () => {
        refresh();
        notifySuccess(`Disconnected "${clientName}"`);
      },
      onError: (err) => notifyError(err, `Couldn't disconnect "${clientName}"`),
    });
  }

  return (
    <div>
      <SectionHeading>Connected apps</SectionHeading>
      <p className="my-4">
        AI tools you connect through seslogin's MCP interface — for example
        Claude Code or a claude.ai custom connector — appear here once you
        approve them. Each one can act as you, with your permissions. Disconnect
        anything you no longer use or don't recognize.
      </p>
      {grants.length === 0 && <p className="my-4">No apps connected yet.</p>}
      {grants.length > 0 && (
        <AdminTable>
          <thead>
            <tr>
              <Th>App</Th>
              <Th>Redirects to</Th>
              <Th>Connected</Th>
              <Th>Last used</Th>
              <Th>Expires if unused</Th>
              <Th></Th>
            </tr>
          </thead>
          <tbody>
            {grants.map((grant, idx) => (
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
                <Td>
                  {formatFullDateTime(new Date(grant.refreshExpiresAt * 1000))}
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
    </div>
  );
}

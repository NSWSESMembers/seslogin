import { useState } from "react";
import { graphql, useFragment, useMutation } from "react-relay";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import type { ApiTokenListQuery } from "./__generated__/ApiTokenListQuery.graphql";
import type { ApiTokenListRevokeMutation } from "./__generated__/ApiTokenListRevokeMutation.graphql";
import type { ApiTokenList_token$key } from "./__generated__/ApiTokenList_token.graphql";
import { formatFullDateTime } from "../../lib/time";
import { useUserInfo } from "../components/useUserInfo";
import { useNotify } from "../components/useNotify";
import { AdminTable, Th, Td } from "../../components/ui/Table";
import { Button, ButtonLink } from "../../components/ui/Button";

function Row(props: {
  token: ApiTokenList_token$key;
  idx: number;
  isDev: boolean;
  locationNames: Map<string, string>;
  now: number;
}) {
  const isDev = props.isDev;
  const { notifyError, notifySuccess } = useNotify();
  const token = useFragment<ApiTokenList_token$key>(
    graphql`
      fragment ApiTokenList_token on ApiToken @throwOnFieldError {
        id
        name
        locationGrants
        readOnly
        createdAt
        expiresAt
        lastUsedAt
      }
    `,
    props.token,
  );

  const [commitMutation, isMutationInFlight] =
    useMutation<ApiTokenListRevokeMutation>(graphql`
      mutation ApiTokenListRevokeMutation($id: ID!) {
        revokeApiToken(id: $id)
      }
    `);

  async function revoke() {
    const yes = confirm(
      `Are you sure you want to revoke the API token "${token.name}"? This cannot be undone — anything using it will stop working immediately.`,
    );
    if (yes) {
      try {
        await new Promise((resolve, reject) => {
          commitMutation({
            variables: { id: token.id },
            onCompleted: resolve,
            onError: reject,
            updater: (store) => {
              store.invalidateStore();
            },
          });
        });
        notifySuccess(`API token "${token.name}" revoked`);
      } catch (err) {
        notifyError(err, `Couldn't revoke API token "${token.name}"`);
      }
    }
  }

  const locationLabel =
    token.locationGrants.length === 0
      ? "(none)"
      : token.locationGrants
          .map((id) => props.locationNames.get(id) ?? id)
          .join(", ");

  const isExpired = token.expiresAt != null && token.expiresAt <= props.now;

  return (
    <tr className={props.idx % 2 === 0 ? "bg-surface-raised" : undefined}>
      {isDev && <Td className="font-mono text-[0.85em]">{token.id}</Td>}
      <Td>
        <span className={isExpired ? "line-through" : undefined}>
          {token.name}
        </span>
      </Td>
      <Td>{locationLabel}</Td>
      <Td>{token.readOnly ? "Yes" : "No"}</Td>
      <Td>{formatFullDateTime(new Date(token.createdAt * 1000))}</Td>
      <Td>
        {token.expiresAt
          ? formatFullDateTime(new Date(token.expiresAt * 1000)) +
            (isExpired ? " (expired)" : "")
          : "Never"}
      </Td>
      <Td>
        {token.lastUsedAt
          ? formatFullDateTime(new Date(token.lastUsedAt * 1000))
          : "Never"}
      </Td>
      <Td options>
        <div className="flex justify-end gap-1">
          <ButtonLink size="row" to={`/admin/api-tokens/${token.id}`}>
            Edit
          </ButtonLink>
          <Button
            size="row"
            variant="danger"
            onClick={revoke}
            disabled={isMutationInFlight}
          >
            Revoke
          </Button>
        </div>
      </Td>
    </tr>
  );
}

export default function ApiTokenList() {
  const { isDev } = useUserInfo();
  const [now] = useState(() => Date.now() / 1000);
  const data = useRetryableLazyLoadQuery<ApiTokenListQuery>(
    graphql`
      query ApiTokenListQuery @throwOnFieldError {
        apiTokens {
          id
          ...ApiTokenList_token
        }
        locations {
          id
          name
        }
      }
    `,
    {},
  );

  const locationNames = new Map(
    data.locations.map((l) => [l.id, l.name] as const),
  );
  const tokens = [...data.apiTokens].sort((a, b) => a.id.localeCompare(b.id));

  return (
    <>
      <p>
        API tokens grant programmatic access to the locations they're scoped to.
        The secret is shown only once, at creation — if it's lost, revoke the
        token and create a new one.
      </p>
      <AdminTable>
        <thead>
          <tr>
            {isDev && <Th>ID</Th>}
            <Th>Name</Th>
            <Th>Locations</Th>
            <Th>Read only</Th>
            <Th>Created</Th>
            <Th>Expires</Th>
            <Th>Last used</Th>
            <Th style={{ width: 140 }}></Th>
          </tr>
        </thead>
        <tbody>
          {tokens.map((token, idx) => (
            <Row
              key={token.id}
              token={token}
              idx={idx}
              isDev={isDev}
              locationNames={locationNames}
              now={now}
            />
          ))}
        </tbody>
      </AdminTable>
    </>
  );
}

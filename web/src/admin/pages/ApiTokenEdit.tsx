import { useState } from "react";
import { useNavigate, useParams } from "react-router";
import { graphql, useMutation } from "react-relay";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import { dateToInputDateTimeLocal, formatFullDateTime } from "../../lib/time";
import type { ApiTokenEditQuery } from "./__generated__/ApiTokenEditQuery.graphql";
import type { ApiTokenEditMutation } from "./__generated__/ApiTokenEditMutation.graphql";
import type { ApiTokenEditRevokeMutation } from "./__generated__/ApiTokenEditRevokeMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import TextInput from "../../components/ui/TextInput";
import { Button } from "../../components/ui/Button";

export default function ApiTokenEdit() {
  const navigate = useNavigate();
  const params = useParams();
  const { notifyError, notifySuccess } = useNotify();
  const id = params.apiTokenId!;

  const data = useRetryableLazyLoadQuery<ApiTokenEditQuery>(
    graphql`
      query ApiTokenEditQuery($id: ID!) @throwOnFieldError {
        apiToken(id: $id) {
          id
          name
          readOnly
          locationGrants
          expiresAt
          createdAt
          lastUsedAt
          revokedAt
        }
        locations {
          id
          name
        }
      }
    `,
    { id },
  );

  const [commitMutation, isMutationInFlight] =
    useMutation<ApiTokenEditMutation>(graphql`
      mutation ApiTokenEditMutation(
        $id: ID!
        $name: String!
        $locationGrants: [String!]!
        $readOnly: Boolean!
        $expiresAt: Int
      ) {
        updateApiToken(
          id: $id
          name: $name
          locationGrants: $locationGrants
          readOnly: $readOnly
          expiresAt: $expiresAt
        ) {
          id
          name
          readOnly
          locationGrants
          expiresAt
        }
      }
    `);

  const [commitRevoke, isRevokeInFlight] =
    useMutation<ApiTokenEditRevokeMutation>(graphql`
      mutation ApiTokenEditRevokeMutation($id: ID!) {
        revokeApiToken(id: $id)
      }
    `);

  async function handleSubmit(formData: FormData) {
    const name = formData.get("name")?.toString().trim() || "";
    const readOnly = formData.get("readOnly") === "on";
    const locationGrants = formData
      .getAll("locations")
      .map((v) => v.toString());
    const expiresAtRaw = formData.get("expiresAt")?.toString() || "";
    const expiresAt = expiresAtRaw
      ? Math.floor(new Date(expiresAtRaw).getTime() / 1000)
      : null;

    try {
      await new Promise((resolve, reject) => {
        commitMutation({
          variables: { id, name, locationGrants, readOnly, expiresAt },
          onCompleted: resolve,
          onError: reject,
          updater: (store) => {
            store.invalidateStore();
          },
        });
      });
    } catch (err) {
      notifyError(err, "Couldn't save API token");
      return;
    }

    notifySuccess("API token saved");
    navigate("/admin/api-tokens");
  }

  async function revoke() {
    const yes = confirm(
      `Are you sure you want to revoke the API token "${token.name}"? This cannot be undone — anything using it will stop working immediately.`,
    );
    if (!yes) return;
    try {
      await new Promise((resolve, reject) => {
        commitRevoke({
          variables: { id },
          onCompleted: resolve,
          onError: reject,
          updater: (store) => {
            store.invalidateStore();
          },
        });
      });
      notifySuccess(`API token "${token.name}" revoked`);
      navigate("/admin/api-tokens");
    } catch (err) {
      notifyError(err, `Couldn't revoke API token "${token.name}"`);
    }
  }

  const locations = [...data.locations].sort((a, b) =>
    a.name.localeCompare(b.name),
  );
  const token = data.apiToken;
  const [readOnly, setReadOnly] = useState(token.readOnly);
  const [selectedLocations, setSelectedLocations] = useState(
    () => new Set(token.locationGrants),
  );
  const [expiresValue, setExpiresValue] = useState(
    token.expiresAt
      ? dateToInputDateTimeLocal(new Date(token.expiresAt * 1000))
      : "",
  );

  const isRevoked = token.revokedAt != null;

  return (
    <>
      <p>Edit the API token's details, then click Save.</p>
      <p className="text-sm text-ink-muted">
        Created {formatFullDateTime(new Date(token.createdAt * 1000))} · Last
        used{" "}
        {token.lastUsedAt
          ? formatFullDateTime(new Date(token.lastUsedAt * 1000))
          : "Never"}
        {isRevoked && (
          <>
            {" "}
            ·{" "}
            <span className="font-bold text-red-600 dark:text-red-400">
              Revoked {formatFullDateTime(new Date(token.revokedAt! * 1000))}
            </span>
          </>
        )}
      </p>

      <form action={handleSubmit}>
        <FieldList>
          <FormField label={<label htmlFor="name">Name</label>}>
            <TextInput
              type="text"
              name="name"
              id="name"
              defaultValue={token.name}
              required
            />
          </FormField>
          <FormField label={<label htmlFor="readOnly">Read only</label>}>
            <input
              type="checkbox"
              name="readOnly"
              id="readOnly"
              checked={readOnly}
              onChange={(e) => setReadOnly(e.target.checked)}
            />
          </FormField>
          <FormField label={<label htmlFor="expiresAt">Expires</label>}>
            <TextInput
              type="datetime-local"
              name="expiresAt"
              id="expiresAt"
              width="auto"
              value={expiresValue}
              onChange={(e) => setExpiresValue(e.target.value)}
            />
          </FormField>
          <FormField label="Locations">
            <a
              href="#"
              onClick={(e) => {
                e.preventDefault();
                setSelectedLocations(new Set());
              }}
            >
              Deselect all
            </a>
            {locations.map((location: { id: string; name: string }) => (
              <div key={location.id}>
                <input
                  type="checkbox"
                  name="locations"
                  id={`location-${location.id}`}
                  value={location.id}
                  checked={selectedLocations.has(location.id)}
                  onChange={(e) =>
                    setSelectedLocations((prev) => {
                      const next = new Set(prev);
                      if (e.target.checked) {
                        next.add(location.id);
                      } else {
                        next.delete(location.id);
                      }
                      return next;
                    })
                  }
                />
                &nbsp;
                <label htmlFor={`location-${location.id}`}>
                  {location.name}
                </label>
              </div>
            ))}
          </FormField>
          <FormField>
            <div className="flex justify-end gap-2 md:justify-start">
              <Button type="submit" disabled={isMutationInFlight}>
                Save
              </Button>
              {!isRevoked && (
                <Button
                  type="button"
                  variant="danger"
                  onClick={revoke}
                  disabled={isRevokeInFlight}
                >
                  Revoke
                </Button>
              )}
            </div>
          </FormField>
        </FieldList>
      </form>
    </>
  );
}

import { useState } from "react";
import { Link } from "react-router";
import { graphql, useMutation } from "react-relay";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import type { ApiTokenNewQuery } from "./__generated__/ApiTokenNewQuery.graphql";
import type { ApiTokenNewMutation } from "./__generated__/ApiTokenNewMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import TextInput from "../../components/ui/TextInput";
import { Button } from "../../components/ui/Button";
import CopyableSecret from "../components/CopyableSecret";

export default function ApiTokenNew() {
  const { notifyError, notifySuccess } = useNotify();
  const [created, setCreated] = useState<{
    name: string;
    secret: string;
  } | null>(null);

  const data = useRetryableLazyLoadQuery<ApiTokenNewQuery>(
    graphql`
      query ApiTokenNewQuery @throwOnFieldError {
        locations {
          id
          name
        }
      }
    `,
    {},
  );

  const [commitMutation, isMutationInFlight] = useMutation<ApiTokenNewMutation>(
    graphql`
      mutation ApiTokenNewMutation(
        $name: String!
        $locationGrants: [String!]!
        $readOnly: Boolean!
        $expiresAt: Int
      ) {
        createApiToken(
          name: $name
          locationGrants: $locationGrants
          readOnly: $readOnly
          expiresAt: $expiresAt
        ) {
          secret
          token {
            id
            name
          }
        }
      }
    `,
  );

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
      const result = await new Promise<ApiTokenNewMutation["response"]>(
        (resolve, reject) => {
          commitMutation({
            variables: { name, locationGrants, readOnly, expiresAt },
            onCompleted: resolve,
            onError: reject,
            updater: (store) => {
              store.invalidateStore();
            },
          });
        },
      );
      setCreated({
        name: result.createApiToken.token.name,
        secret: result.createApiToken.secret,
      });
      notifySuccess(`API token "${name}" created`);
    } catch (err) {
      notifyError(err, "Couldn't create API token");
    }
  }

  if (created) {
    return (
      <>
        <p>
          API token <strong>{created.name}</strong> was created. Copy the secret
          below now — for security, it's shown only this once and can't be
          retrieved again. If it's lost, revoke this token and create a new one.
        </p>
        <CopyableSecret secret={created.secret} />
        <p>
          <Link to="/admin/api-tokens">Back to API tokens</Link>
        </p>
      </>
    );
  }

  const locations = [...data.locations].sort((a, b) =>
    a.name.localeCompare(b.name),
  );

  return (
    <>
      <p>Enter the details of the new API token in the form below.</p>

      <form action={handleSubmit}>
        <FieldList>
          <FormField label={<label htmlFor="name">Name</label>}>
            <TextInput type="text" name="name" id="name" required />
          </FormField>
          <FormField label={<label htmlFor="readOnly">Read only</label>}>
            <input type="checkbox" name="readOnly" id="readOnly" />
          </FormField>
          <FormField label={<label htmlFor="expiresAt">Expires</label>}>
            <TextInput
              type="datetime-local"
              name="expiresAt"
              id="expiresAt"
              width="auto"
            />
          </FormField>
          <FormField label="Locations">
            {locations.map((location: { id: string; name: string }) => (
              <div key={location.id}>
                <input
                  type="checkbox"
                  name="locations"
                  id={`location-${location.id}`}
                  value={location.id}
                />
                &nbsp;
                <label htmlFor={`location-${location.id}`}>
                  {location.name}
                </label>
              </div>
            ))}
          </FormField>
          <FormField>
            <Button type="submit" disabled={isMutationInFlight}>
              Create
            </Button>
          </FormField>
        </FieldList>
      </form>
    </>
  );
}

import { useState } from "react";
import { useNavigate } from "react-router";
import { graphql, useMutation } from "react-relay";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import type { UserNewQuery } from "./__generated__/UserNewQuery.graphql";
import type { UserNewMutation } from "./__generated__/UserNewMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import TextInput from "../../components/ui/TextInput";
import { Button } from "../../components/ui/Button";
import LocationRolesField from "../components/LocationRolesField";
import {
  splitLocationRoles,
  type LocationRoleEntry,
} from "../components/locationRoles";

export default function NewUser() {
  const navigate = useNavigate();
  const { notifyError, notifySuccess } = useNotify();
  const data = useRetryableLazyLoadQuery<UserNewQuery>(
    graphql`
      query UserNewQuery @throwOnFieldError {
        locations {
          id
          name
        }
      }
    `,
    {},
  );

  const [commitMutation, isMutationInFlight] = useMutation<UserNewMutation>(
    graphql`
      mutation UserNewMutation(
        $email: String!
        $isSuper: Boolean!
        $locationGrants: [String!]!
        $readOnlyLocationGrants: [String!]!
      ) {
        createUser(
          email: $email
          isSuper: $isSuper
          locationGrants: $locationGrants
          readOnlyLocationGrants: $readOnlyLocationGrants
        ) {
          id
          email
        }
      }
    `,
  );

  const [roles, setRoles] = useState<LocationRoleEntry[]>([]);

  async function handleSubmit(formData: FormData) {
    const email = formData.get("email")?.toString() || "";
    const isSuper = formData.get("super") === "on";
    const { locationGrants, readOnlyLocationGrants } =
      splitLocationRoles(roles);

    try {
      await new Promise((resolve, reject) => {
        commitMutation({
          variables: { email, isSuper, locationGrants, readOnlyLocationGrants },
          onCompleted: resolve,
          onError: reject,
          updater: (store) => {
            store.invalidateStore();
          },
        });
      });
    } catch (err) {
      notifyError(err, "Couldn't create user");
      return;
    }

    notifySuccess("User created");
    navigate("/admin/users");
  }

  const locations = [...data.locations].sort((a, b) =>
    a.name.localeCompare(b.name),
  );

  return (
    <>
      <p className="my-4">
        Enter the details of the new user in the form below.
      </p>

      <form action={handleSubmit}>
        <FieldList>
          <FormField label={<label htmlFor="email">Email</label>}>
            <TextInput type="email" name="email" id="email" required />
          </FormField>
          <FormField label={<label htmlFor="super">Super</label>}>
            <input type="checkbox" name="super" id="super" />
          </FormField>
          <FormField label={<label htmlFor="locations">Locations</label>}>
            <LocationRolesField
              id="locations"
              locations={locations.map((l) => ({ id: l.id, name: l.name }))}
              value={roles}
              onChange={setRoles}
            />
          </FormField>
          <FormField>
            <Button type="submit" disabled={isMutationInFlight}>
              Save
            </Button>
          </FormField>
        </FieldList>
      </form>
    </>
  );
}

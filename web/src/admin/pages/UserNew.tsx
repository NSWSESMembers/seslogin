import { useNavigate } from "react-router";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import type { UserNewQuery } from "./__generated__/UserNewQuery.graphql";
import type { UserNewMutation } from "./__generated__/UserNewMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import TextInput from "../../components/ui/TextInput";
import { Button } from "../../components/ui/Button";

export default function NewUser() {
  const navigate = useNavigate();
  const { notifyError, notifySuccess } = useNotify();
  const data = useLazyLoadQuery<UserNewQuery>(
    graphql`
      query UserNewQuery {
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
      ) {
        createUser(
          email: $email
          isSuper: $isSuper
          locationGrants: $locationGrants
        ) {
          id
          email
        }
      }
    `,
  );

  async function handleSubmit(formData: FormData) {
    const email = formData.get("email")?.toString() || "";
    const isSuper = formData.get("super") === "on";
    const locationGrants = formData
      .getAll("locations")
      .map((v) => v.toString());

    try {
      await new Promise((resolve, reject) => {
        commitMutation({
          variables: { email, isSuper, locationGrants },
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
      <p>Enter the details of the new user in the form below.</p>
      {/* {error && <p className="font-bold text-red-600">Error: {error.message}</p>} */}

      <form action={handleSubmit}>
        <FieldList>
          <FormField label={<label htmlFor="email">Email</label>}>
            <TextInput type="email" name="email" id="email" required />
          </FormField>
          <FormField label={<label htmlFor="super">Super</label>}>
            <input type="checkbox" name="super" id="super" />
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
              Save
            </Button>
          </FormField>
        </FieldList>
      </form>
    </>
  );
}

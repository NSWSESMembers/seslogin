import { useState } from "react";
import { useNavigate, useParams } from "react-router";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import type { UserEditQuery } from "./__generated__/UserEditQuery.graphql";
import type { UserEditMutation } from "./__generated__/UserEditMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import TextInput from "../../components/ui/TextInput";
import { Button } from "../../components/ui/Button";

export default function UserEdit() {
  const navigate = useNavigate();
  const params = useParams();
  const { notifyError, notifySuccess } = useNotify();
  const id = params.userId!;

  const data = useLazyLoadQuery<UserEditQuery>(
    graphql`
      query UserEditQuery($id: ID!) {
        user(id: $id) {
          id
          email
          isSuper
          isDev
          enabled
          locationGrantIds
        }
        locations {
          id
          name
        }
      }
    `,
    { id },
  );

  const [commitMutation, isMutationInFlight] = useMutation<UserEditMutation>(
    graphql`
      mutation UserEditMutation(
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
          email
          isSuper
          isDev
          locationGrantIds
        }
      }
    `,
  );

  async function handleSubmit(formData: FormData) {
    const email = formData.get("email")?.toString() || "";
    const isSuper = formData.get("super") === "on";
    const isDev = formData.get("dev") === "on";
    const enabled = formData.get("enabled") === "on";
    const locationGrants = formData
      .getAll("locations")
      .map((v) => v.toString());

    try {
      await new Promise((resolve, reject) => {
        commitMutation({
          variables: {
            id,
            email,
            isSuper,
            isDev,
            locationGrants,
            enabled,
          },
          onCompleted: resolve,
          onError: reject,
          updater: (store) => {
            store.invalidateStore();
          },
        });
      });
    } catch (err) {
      notifyError(err, "Couldn't save user");
      return;
    }

    notifySuccess("User saved");
    navigate("/admin/users");
  }

  const locations = [...data.locations].sort((a, b) =>
    a.name.localeCompare(b.name),
  );
  const user = data.user;
  const [isSuper, setIsSuper] = useState(user.isSuper);
  const [isDev, setIsDev] = useState(user.isDev);
  const [enabled, setEnabled] = useState(user.enabled);
  const [selectedLocations, setSelectedLocations] = useState(
    () => new Set(user.locationGrantIds),
  );

  return (
    <>
      <p>Edit the member's details, then click Save.</p>
      {/* {updateError && <p className="font-bold text-red-600">Error: {updateError.message}</p>} */}

      <form action={handleSubmit}>
        <FieldList>
          <FormField label={<label htmlFor="email">Email</label>}>
            <TextInput
              type="email"
              name="email"
              id="email"
              defaultValue={user.email}
              required
            />
          </FormField>
          <FormField label={<label htmlFor="super">Super</label>}>
            <input
              type="checkbox"
              name="super"
              id="super"
              checked={isSuper}
              onChange={(e) => setIsSuper(e.target.checked)}
            />
          </FormField>
          <FormField label={<label htmlFor="dev">Dev</label>}>
            <input
              type="checkbox"
              name="dev"
              id="dev"
              checked={isDev}
              onChange={(e) => setIsDev(e.target.checked)}
            />
          </FormField>
          <FormField label={<label htmlFor="enabled">Enabled</label>}>
            <input
              type="checkbox"
              name="enabled"
              id="enabled"
              checked={enabled}
              onChange={(e) => setEnabled(e.target.checked)}
            />
          </FormField>
          {!isSuper && (
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
          )}
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

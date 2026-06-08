import { useState } from "react";
import { useNavigate, useParams } from "react-router";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import type { UserEditQuery } from "./__generated__/UserEditQuery.graphql";
import type { UserEditMutation } from "./__generated__/UserEditMutation.graphql";
import { useNotify } from "../components/useNotify";

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
      {/* {updateError && <p className="error">Error: {updateError.message}</p>} */}

      <form action={handleSubmit}>
        <dl>
          <dt>
            <label htmlFor="email" className="required">
              Email
            </label>
          </dt>
          <dd>
            <input
              type="email"
              name="email"
              id="email"
              defaultValue={user.email}
              required
            />
          </dd>
          <dt>
            <label htmlFor="super">Super</label>
          </dt>
          <dd>
            <input
              type="checkbox"
              name="super"
              id="super"
              checked={isSuper}
              onChange={(e) => setIsSuper(e.target.checked)}
            />
          </dd>
          <dt>
            <label htmlFor="dev">Dev</label>
          </dt>
          <dd>
            <input
              type="checkbox"
              name="dev"
              id="dev"
              checked={isDev}
              onChange={(e) => setIsDev(e.target.checked)}
            />
          </dd>
          <dt>
            <label htmlFor="enabled">Enabled</label>
          </dt>
          <dd>
            <input
              type="checkbox"
              name="enabled"
              id="enabled"
              checked={enabled}
              onChange={(e) => setEnabled(e.target.checked)}
            />
          </dd>
          {!isSuper && (
            <>
              <dt>Locations</dt>
              <dd>
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
              </dd>
            </>
          )}
          <dt>&nbsp;</dt>
          <dd>
            <button type="submit" disabled={isMutationInFlight}>
              Save
            </button>
          </dd>
        </dl>
      </form>
    </>
  );
}

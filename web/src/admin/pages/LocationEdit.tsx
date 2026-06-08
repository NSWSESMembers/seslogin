import { useNavigate, useParams } from "react-router";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import type { LocationEditQuery } from "./__generated__/LocationEditQuery.graphql";
import type { LocationEditMutation } from "./__generated__/LocationEditMutation.graphql";
import { useNotify } from "../components/useNotify";

export default function EditLocation() {
  const params = useParams();
  const navigate = useNavigate();
  const { notifyError, notifySuccess } = useNotify();
  const id = params.locationId!;

  const data = useLazyLoadQuery<LocationEditQuery>(
    graphql`
      query LocationEditQuery($id: ID!) {
        location(id: $id) {
          id
          name
          enabled
          nitcEnabled
        }
      }
    `,
    { id },
  );

  const [commitMutation, isMutationInFlight] =
    useMutation<LocationEditMutation>(graphql`
      mutation LocationEditMutation(
        $id: ID!
        $name: String!
        $enabled: Boolean!
        $nitcEnabled: Int
      ) {
        updateLocation(
          id: $id
          name: $name
          enabled: $enabled
          nitcEnabled: $nitcEnabled
        ) {
          id
          name
          enabled
          nitcEnabled
        }
      }
    `);

  const location = data.location;

  async function handleSubmit(formData: FormData) {
    const name = formData.get("name")?.toString() || "";
    const enabled = formData.get("enabled") === "on";
    const nitcEnabledDate = formData.get("nitcEnabled")?.toString() || "";
    const nitcEnabled = nitcEnabledDate
      ? Math.floor(new Date(nitcEnabledDate + "T00:00:00Z").getTime() / 1000)
      : null;

    try {
      await new Promise((resolve, reject) => {
        commitMutation({
          variables: { id: location.id, name, enabled, nitcEnabled },
          onCompleted: resolve,
          onError: reject,
          updater: (store) => {
            store.invalidateStore();
          },
        });
      });
    } catch (err) {
      notifyError(err, "Couldn't save location");
      return;
    }

    notifySuccess("Location saved");
    navigate("/admin/locations");
  }

  return (
    <>
      <p>Edit the location's details, then click Save.</p>
      {/* {updateError && <p className="error">Error: {updateError.message}</p>} */}

      <form action={handleSubmit}>
        <dl>
          <dt>
            <label htmlFor="name" className="required">
              Name
            </label>
          </dt>
          <dd>
            <input
              type="text"
              name="name"
              id="name"
              defaultValue={location.name}
              required
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
              defaultChecked={location.enabled}
            />
          </dd>
          <dt>
            <label htmlFor="nitcEnabled">NITC Export From</label>
          </dt>
          <dd>
            <input
              type="date"
              name="nitcEnabled"
              id="nitcEnabled"
              defaultValue={
                location.nitcEnabled
                  ? new Date(location.nitcEnabled * 1000)
                      .toISOString()
                      .slice(0, 10)
                  : ""
              }
            />
          </dd>
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

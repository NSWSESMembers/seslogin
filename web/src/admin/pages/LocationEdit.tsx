import { useNavigate, useParams } from "react-router";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import type { LocationEditQuery } from "./__generated__/LocationEditQuery.graphql";
import type { LocationEditMutation } from "./__generated__/LocationEditMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import TextInput from "../../components/ui/TextInput";
import { Button } from "../../components/ui/Button";

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
      {/* {updateError && <p className="font-bold text-red-600">Error: {updateError.message}</p>} */}

      <form action={handleSubmit}>
        <FieldList>
          <FormField label={<label htmlFor="name">Name</label>}>
            <TextInput
              type="text"
              name="name"
              id="name"
              defaultValue={location.name}
              required
            />
          </FormField>
          <FormField label={<label htmlFor="enabled">Enabled</label>}>
            <input
              type="checkbox"
              name="enabled"
              id="enabled"
              defaultChecked={location.enabled}
            />
          </FormField>
          <FormField
            label={<label htmlFor="nitcEnabled">NITC Export From</label>}
          >
            <TextInput
              type="date"
              name="nitcEnabled"
              id="nitcEnabled"
              width="medium"
              defaultValue={
                location.nitcEnabled
                  ? new Date(location.nitcEnabled * 1000)
                      .toISOString()
                      .slice(0, 10)
                  : ""
              }
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

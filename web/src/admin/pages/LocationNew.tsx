import { useMutation, graphql } from "react-relay";
import { useNavigate } from "react-router";
import { type LocationNewMutation } from "./__generated__/LocationNewMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import TextInput from "../../components/ui/TextInput";
import { Button } from "../../components/ui/Button";

export default function LocationNew() {
  const { notifyError, notifySuccess } = useNotify();
  const [commitMutation, isMutationInFlight] = useMutation<LocationNewMutation>(
    graphql`
      mutation LocationNewMutation($name: String!, $nitcEnabled: Int) {
        createLocation(name: $name, nitcEnabled: $nitcEnabled) {
          id
          ...LocationList_item
        }
      }
    `,
  );
  const navigate = useNavigate();

  async function handleSubmit(formData: FormData) {
    const name = formData.get("name")?.toString() || "";
    const nitcEnabledDate = formData.get("nitcEnabled")?.toString() || "";
    const nitcEnabled = nitcEnabledDate
      ? Math.floor(new Date(nitcEnabledDate + "T00:00:00Z").getTime() / 1000)
      : null;

    try {
      await new Promise((resolve, reject) => {
        commitMutation({
          variables: { name, nitcEnabled },
          onCompleted: (data) => {
            resolve(data);
          },
          onError: (err) => {
            reject(err);
          },
          updater: (store) => {
            store.invalidateStore();
          },
        });
      });
    } catch (err) {
      notifyError(err, "Couldn't create location");
      return;
    }

    // this should only happen if no error was thrown
    notifySuccess("Location created");
    navigate("/admin/locations");
  }

  return (
    <>
      <p>Enter the details of the new location in the form below.</p>
      {/* {error && <p className="font-bold text-red-600">Error: {error.message}</p>} */}

      <form action={handleSubmit}>
        <FieldList>
          <FormField label={<label htmlFor="name">Name</label>}>
            <TextInput type="text" name="name" id="name" required />
          </FormField>
          <FormField label={<label htmlFor="nitcEnabled">NITC Export</label>}>
            <input type="checkbox" name="nitcEnabled" id="nitcEnabled" />
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

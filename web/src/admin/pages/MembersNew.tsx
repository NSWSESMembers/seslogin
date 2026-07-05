import { graphql, useMutation } from "react-relay";
import { useNavigate } from "react-router";
import useSelectedLocation from "../components/useSelectedLocation";
import type { MembersNewMutation } from "./__generated__/MembersNewMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import TextInput from "../../components/ui/TextInput";
import { Button } from "../../components/ui/Button";

export default function MembersNew() {
  const { notifyError, notifySuccess } = useNotify();
  const [commitMutation, isMutationInFlight] = useMutation<MembersNewMutation>(
    graphql`
      mutation MembersNewMutation(
        $firstName: String!
        $lastName: String!
        $memberNumber: String!
        $locationId: ID!
      ) {
        createPerson(
          firstName: $firstName
          lastName: $lastName
          memberNumber: $memberNumber
          locationId: $locationId
        ) {
          id
          firstName
          lastName
          memberNumber
        }
      }
    `,
  );

  const selectedLocation = useSelectedLocation();
  const locationId = selectedLocation.id;
  const navigate = useNavigate();

  async function handleSubmit(formData: FormData) {
    const firstName = formData.get("givenname")?.toString() || "";
    const lastName = formData.get("surname")?.toString() || "";
    const memberNumber = formData.get("serialnumber")?.toString() || "";
    try {
      await new Promise((resolve, reject) => {
        commitMutation({
          variables: { firstName, lastName, memberNumber, locationId },
          onCompleted: resolve,
          onError: reject,
          updater: (store) => {
            const location = store.get(locationId);
            location?.invalidateRecord();
          },
        });
      });
    } catch (err) {
      notifyError(err, "Couldn't create member");
      return;
    }

    notifySuccess("Member created");
    navigate("/admin/members");
  }

  return (
    <>
      <p>Enter the details of the new member in the form below.</p>
      {/* {error && <p className="font-bold text-red-600">Error: {error.message}</p>} */}

      <form action={handleSubmit}>
        <FieldList>
          <FormField label={<label htmlFor="givenname">Given name</label>}>
            <TextInput type="text" name="givenname" id="givenname" required />
          </FormField>
          <FormField label={<label htmlFor="surname">Surname</label>}>
            <TextInput type="text" name="surname" id="surname" required />
          </FormField>
          <FormField label={<label htmlFor="serialnumber">SES ID</label>}>
            <TextInput
              type="text"
              name="serialnumber"
              id="serialnumber"
              width="medium"
              required
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

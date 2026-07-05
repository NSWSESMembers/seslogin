import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import { useNavigate, useParams } from "react-router";
import useSelectedLocation from "../components/useSelectedLocation";
import type { MembersEditQuery } from "./__generated__/MembersEditQuery.graphql";
import type { MembersEditMutation } from "./__generated__/MembersEditMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import TextInput from "../../components/ui/TextInput";
import { Button } from "../../components/ui/Button";

export default function MembersEdit() {
  const params = useParams();
  const navigate = useNavigate();
  const { notifyError, notifySuccess } = useNotify();
  const selectedLocation = useSelectedLocation();
  const locationId = selectedLocation.id;
  const data = useLazyLoadQuery<MembersEditQuery>(
    graphql`
      query MembersEditQuery($id: ID!) {
        person(id: $id) {
          id
          firstName
          lastName
          memberNumber
        }
      }
    `,
    { id: params.memberId! },
  );

  const [commitMutation, isMutationInFlight] = useMutation<MembersEditMutation>(
    graphql`
      mutation MembersEditMutation(
        $id: ID!
        $firstName: String!
        $lastName: String!
        $memberNumber: String!
      ) {
        updatePerson(
          id: $id
          firstName: $firstName
          lastName: $lastName
          memberNumber: $memberNumber
        ) {
          id
          firstName
          lastName
          memberNumber
        }
      }
    `,
  );

  async function handleSubmit(formData: FormData) {
    const firstName = formData.get("givenname")?.toString() || "";
    const lastName = formData.get("surname")?.toString() || "";
    const memberNumber = formData.get("serialnumber")?.toString() || "";
    try {
      await new Promise((resolve, reject) => {
        commitMutation({
          variables: { id: person.id, firstName, lastName, memberNumber },
          onCompleted: resolve,
          onError: reject,
          updater: (store) => {
            const location = store.get(locationId);
            location?.invalidateRecord();
          },
        });
      });
    } catch (err) {
      notifyError(err, "Couldn't save member");
      return;
    }
    notifySuccess("Member saved");
    navigate("/admin/members");
  }

  const person = data.person;

  return (
    <>
      <p>Edit the member's details, then click Save.</p>
      {/* {updateError && <p className="font-bold text-red-600">Error: {updateError.message}</p>} */}

      <form action={handleSubmit}>
        <FieldList>
          <FormField label={<label htmlFor="givenname">Given name</label>}>
            <TextInput
              type="text"
              name="givenname"
              id="givenname"
              defaultValue={person?.firstName}
              required
            />
          </FormField>
          <FormField label={<label htmlFor="surname">Surname</label>}>
            <TextInput
              type="text"
              name="surname"
              id="surname"
              defaultValue={person.lastName}
              required
            />
          </FormField>
          <FormField label={<label htmlFor="serialnumber">SES ID</label>}>
            <TextInput
              type="text"
              name="serialnumber"
              id="serialnumber"
              width="medium"
              defaultValue={person.memberNumber || ""}
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

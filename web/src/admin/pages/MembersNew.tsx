import { graphql, useMutation } from "react-relay";
import { useNavigate } from "react-router";
import useSelectedLocation from "../components/useSelectedLocation";
import type { MembersNewMutation } from "./__generated__/MembersNewMutation.graphql";
import { useNotify } from "../components/useNotify";

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
      {/* {error && <p className="error">Error: {error.message}</p>} */}

      <form action={handleSubmit}>
        <dl>
          <dt>
            <label htmlFor="givenname" className="required">
              Given name
            </label>
          </dt>
          <dd>
            <input type="text" name="givenname" id="givenname" required />
          </dd>
          <dt>
            <label htmlFor="surname" className="required">
              Surname
            </label>
          </dt>
          <dd>
            <input type="text" name="surname" id="surname" required />
          </dd>
          <dt>
            <label htmlFor="serialnumber" className="required">
              SES ID
            </label>
          </dt>
          <dd>
            <input
              type="text"
              name="serialnumber"
              id="serialnumber"
              className="medium"
              required
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

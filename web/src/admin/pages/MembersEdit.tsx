import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import { useNavigate, useParams } from "react-router";
import useSelectedLocation from "../components/useSelectedLocation";
import type { MembersEditQuery } from "./__generated__/MembersEditQuery.graphql";
import type { MembersEditMutation } from "./__generated__/MembersEditMutation.graphql";
import { useNotify } from "../components/useNotify";

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
      {/* {updateError && <p className="error">Error: {updateError.message}</p>} */}

      <form action={handleSubmit}>
        <dl>
          <dt>
            <label htmlFor="givenname" className="required">
              Given name
            </label>
          </dt>
          <dd>
            <input
              type="text"
              name="givenname"
              id="givenname"
              defaultValue={person?.firstName}
              required
            />
          </dd>
          <dt>
            <label htmlFor="surname" className="required">
              Surname
            </label>
          </dt>
          <dd>
            <input
              type="text"
              name="surname"
              id="surname"
              defaultValue={person.lastName}
              required
            />
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
              defaultValue={person.memberNumber || ""}
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

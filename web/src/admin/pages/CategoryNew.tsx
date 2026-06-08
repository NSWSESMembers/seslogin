import { useNavigate } from "react-router";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import type { CategoryNewQuery } from "./__generated__/CategoryNewQuery.graphql";
import type { CategoryNewMutation } from "./__generated__/CategoryNewMutation.graphql";
import { useNotify } from "../components/useNotify";

export default function CategoryNew() {
  const { notifyError, notifySuccess } = useNotify();
  const [commitMutation, isMutationInFlight] = useMutation<CategoryNewMutation>(
    graphql`
      mutation CategoryNewMutation(
        $name: String!
        $nitcGroupId: String
        $nitcParticipantType: String
      ) {
        createCategory(
          name: $name
          nitcGroupId: $nitcGroupId
          nitcParticipantType: $nitcParticipantType
        ) {
          id
          name
        }
      }
    `,
  );
  const navigate = useNavigate();

  const data = useLazyLoadQuery<CategoryNewQuery>(
    graphql`
      query CategoryNewQuery {
        nitcGroups {
          id
          nitcType
        }
        ses_participant_types
      }
    `,
    {},
  );

  async function handleSubmit(formData: FormData) {
    const name = formData.get("name")?.toString() || "";
    const nitcGroupId = formData.get("nitcGroupId")?.toString() || null;
    const nitcParticipantType =
      formData.get("nitcParticipantType")?.toString() || null;

    try {
      await new Promise((resolve, reject) => {
        commitMutation({
          variables: { name, nitcGroupId, nitcParticipantType },
          onCompleted: resolve,
          onError: reject,
          updater: (store) => {
            store.invalidateStore();
          },
        });
      });
    } catch (err) {
      notifyError(err, "Couldn't create category");
      return;
    }

    notifySuccess("Category created");
    navigate("/admin/categories");
  }

  const sortedGroups = [...data.nitcGroups].sort((a, b) =>
    a.id.localeCompare(b.id),
  );

  return (
    <>
      <p>Enter the details of the new category in the form below.</p>
      <form action={handleSubmit}>
        <dl>
          <dt>
            <label htmlFor="name" className="required">
              Name
            </label>
          </dt>
          <dd>
            <input type="text" name="name" id="name" required />
          </dd>
          <dt>
            <label htmlFor="nitcGroupId">NITC Group</label>
          </dt>
          <dd>
            <select name="nitcGroupId" id="nitcGroupId">
              <option value="">None</option>
              {sortedGroups.map((g) => (
                <option key={g.id} value={g.id}>
                  {g.id} — {g.nitcType}
                </option>
              ))}
            </select>
          </dd>
          <dt>
            <label htmlFor="nitcParticipantType">NITC Participant Type</label>
          </dt>
          <dd>
            <select name="nitcParticipantType" id="nitcParticipantType">
              <option value="">None</option>
              {[...data.ses_participant_types].sort().map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </select>
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

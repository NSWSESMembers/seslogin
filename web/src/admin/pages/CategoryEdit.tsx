import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import { useNavigate, useParams } from "react-router";
import type { CategoryEditMutation } from "./__generated__/CategoryEditMutation.graphql";
import type { CategoryEditQuery } from "./__generated__/CategoryEditQuery.graphql";
import { useNotify } from "../components/useNotify";

export default function CategoryEdit() {
  const navigate = useNavigate();
  const { notifyError, notifySuccess } = useNotify();
  const params = useParams();
  const id = params.categoryId!;
  const data = useLazyLoadQuery<CategoryEditQuery>(
    graphql`
      query CategoryEditQuery($id: ID!) {
        category(id: $id) {
          id
          name
          enabled
          nitcGroupId
          nitcParticipantType
        }
        nitcGroups {
          id
          nitcType
        }
        ses_participant_types
      }
    `,
    { id },
  );

  const [commitMutation, isMutationInFlight] =
    useMutation<CategoryEditMutation>(graphql`
      mutation CategoryEditMutation(
        $id: ID!
        $name: String!
        $enabled: Boolean!
        $nitcGroupId: String
        $nitcParticipantType: String
      ) {
        updateCategory(
          id: $id
          name: $name
          enabled: $enabled
          nitcGroupId: $nitcGroupId
          nitcParticipantType: $nitcParticipantType
        ) {
          id
          name
          enabled
          nitcGroupId
          nitcParticipantType
        }
      }
    `);

  async function handleSubmit(formData: FormData) {
    const name = formData.get("name")?.toString() || "";
    const enabled = formData.get("enabled") === "on";
    const nitcGroupId = formData.get("nitcGroupId")?.toString() || null;
    const nitcParticipantType =
      formData.get("nitcParticipantType")?.toString() || null;

    try {
      await new Promise((resolve, reject) => {
        commitMutation({
          variables: { id, name, enabled, nitcGroupId, nitcParticipantType },
          onCompleted: resolve,
          onError: reject,
          updater: (store) => {
            store.invalidateStore();
          },
        });
      });
    } catch (err) {
      notifyError(err, "Couldn't save category");
      return;
    }
    notifySuccess("Category saved");
    navigate("/admin/categories");
  }

  const category = data.category;
  const sortedGroups = [...data.nitcGroups].sort((a, b) =>
    a.id.localeCompare(b.id),
  );

  return (
    <>
      <p>Edit the category&apos;s details, then click Save.</p>
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
              defaultValue={category.name}
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
              defaultChecked={category.enabled}
            />
          </dd>
          <dt>
            <label htmlFor="nitcGroupId">NITC Group</label>
          </dt>
          <dd>
            <select
              name="nitcGroupId"
              id="nitcGroupId"
              defaultValue={category.nitcGroupId ?? ""}
            >
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
            <select
              name="nitcParticipantType"
              id="nitcParticipantType"
              defaultValue={category.nitcParticipantType ?? ""}
            >
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

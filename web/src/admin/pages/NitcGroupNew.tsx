import { useNavigate } from "react-router";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import type { NitcGroupNewQuery } from "./__generated__/NitcGroupNewQuery.graphql";
import type { NitcGroupNewMutation } from "./__generated__/NitcGroupNewMutation.graphql";
import { useNotify } from "../components/useNotify";

export default function NitcGroupNew() {
  const navigate = useNavigate();
  const { notifyError, notifySuccess } = useNotify();

  const data = useLazyLoadQuery<NitcGroupNewQuery>(
    graphql`
      query NitcGroupNewQuery {
        ses_nonincident_types
        ses_nonincident_tags {
          id
          name
        }
      }
    `,
    {},
  );

  const [commitMutation, isMutationInFlight] =
    useMutation<NitcGroupNewMutation>(graphql`
      mutation NitcGroupNewMutation(
        $id: String
        $nitcType: String!
        $nitcTagIds: [Int!]!
      ) {
        createNitcGroup(id: $id, nitcType: $nitcType, nitcTagIds: $nitcTagIds) {
          id
          nitcType
        }
      }
    `);

  async function handleSubmit(formData: FormData) {
    const id = formData.get("id")?.toString() || null;
    const nitcType = formData.get("nitcType")?.toString() || "";
    const nitcTagIds = data.ses_nonincident_tags
      .filter((t) => formData.get(`tag_${t.id}`) === "on")
      .map((t) => parseInt(t.id, 10));

    try {
      await new Promise((resolve, reject) => {
        commitMutation({
          variables: { id, nitcType, nitcTagIds },
          onCompleted: resolve,
          onError: reject,
          updater: (store) => {
            store.invalidateStore();
          },
        });
      });
    } catch (err) {
      notifyError(err, "Couldn't create NITC group");
      return;
    }

    notifySuccess("NITC group created");
    navigate("/admin/categories/nitc-groups");
  }

  return (
    <>
      <p>Enter the details of the new NITC group below.</p>
      <form action={handleSubmit}>
        <dl>
          <dt>
            <label htmlFor="id" className="required">
              ID
            </label>
          </dt>
          <dd>
            <input
              type="text"
              name="id"
              id="id"
              placeholder="auto-generated if blank"
            />
          </dd>
          <dt>
            <label htmlFor="nitcType" className="required">
              NITC Type
            </label>
          </dt>
          <dd>
            <select name="nitcType" id="nitcType" required>
              <option value="">Select...</option>
              {[...data.ses_nonincident_types].sort().map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </select>
          </dd>
          <dt>SES Tags</dt>
          <dd>
            {[...data.ses_nonincident_tags]
              .sort((a, b) => a.name.localeCompare(b.name))
              .map((tag) => (
                <div key={tag.id}>
                  <label>
                    <input type="checkbox" name={`tag_${tag.id}`} /> {tag.name}
                  </label>
                </div>
              ))}
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

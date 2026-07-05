import { useNavigate, useParams } from "react-router";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import type { NitcGroupEditQuery } from "./__generated__/NitcGroupEditQuery.graphql";
import type { NitcGroupEditMutation } from "./__generated__/NitcGroupEditMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import Select from "../../components/ui/Select";
import { Button } from "../../components/ui/Button";

export default function NitcGroupEdit() {
  const navigate = useNavigate();
  const params = useParams();
  const { notifyError, notifySuccess } = useNotify();
  const id = params.nitcGroupId!;

  const data = useLazyLoadQuery<NitcGroupEditQuery>(
    graphql`
      query NitcGroupEditQuery($id: ID!) {
        nitcGroup(id: $id) {
          id
          nitcType
          sesTags {
            id
            name
          }
        }
        ses_nonincident_types
        ses_nonincident_tags {
          id
          name
        }
      }
    `,
    { id },
  );

  const [commitMutation, isMutationInFlight] =
    useMutation<NitcGroupEditMutation>(graphql`
      mutation NitcGroupEditMutation(
        $id: ID!
        $nitcType: String!
        $nitcTagIds: [Int!]!
      ) {
        updateNitcGroup(id: $id, nitcType: $nitcType, nitcTagIds: $nitcTagIds) {
          id
          nitcType
          sesTags {
            id
            name
          }
        }
      }
    `);

  async function handleSubmit(formData: FormData) {
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
      notifyError(err, "Couldn't save NITC group");
      return;
    }

    notifySuccess("NITC group saved");
    navigate("/admin/categories/nitc-groups");
  }

  const group = data.nitcGroup;
  const currentTagIds = new Set(group.sesTags.map((t) => t.id));

  return (
    <>
      <p>Edit the NITC group&apos;s details, then click Save.</p>
      <form action={handleSubmit}>
        <FieldList>
          <FormField label="ID">
            <code>{id}</code>
          </FormField>
          <FormField label={<label htmlFor="nitcType">NITC Type</label>}>
            <Select
              name="nitcType"
              id="nitcType"
              defaultValue={group.nitcType}
              required
            >
              {[...data.ses_nonincident_types].sort().map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </Select>
          </FormField>
          <FormField label="SES Tags">
            {[...data.ses_nonincident_tags]
              .sort((a, b) => a.name.localeCompare(b.name))
              .map((tag) => (
                <div key={tag.id}>
                  <label>
                    <input
                      type="checkbox"
                      name={`tag_${tag.id}`}
                      defaultChecked={currentTagIds.has(tag.id)}
                    />{" "}
                    {tag.name}
                  </label>
                </div>
              ))}
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

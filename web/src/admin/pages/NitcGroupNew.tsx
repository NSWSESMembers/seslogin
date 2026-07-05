import { useNavigate } from "react-router";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import type { NitcGroupNewQuery } from "./__generated__/NitcGroupNewQuery.graphql";
import type { NitcGroupNewMutation } from "./__generated__/NitcGroupNewMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import TextInput from "../../components/ui/TextInput";
import Select from "../../components/ui/Select";
import { Button } from "../../components/ui/Button";

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
        <FieldList>
          <FormField label={<label htmlFor="id">ID</label>}>
            <TextInput
              type="text"
              name="id"
              id="id"
              placeholder="auto-generated if blank"
            />
          </FormField>
          <FormField label={<label htmlFor="nitcType">NITC Type</label>}>
            <Select name="nitcType" id="nitcType" required>
              <option value="">Select...</option>
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
                    <input type="checkbox" name={`tag_${tag.id}`} /> {tag.name}
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

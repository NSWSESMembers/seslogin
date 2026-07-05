import { useNavigate } from "react-router";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import type { CategoryNewQuery } from "./__generated__/CategoryNewQuery.graphql";
import type { CategoryNewMutation } from "./__generated__/CategoryNewMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import TextInput from "../../components/ui/TextInput";
import Select from "../../components/ui/Select";
import { Button } from "../../components/ui/Button";

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
        <FieldList>
          <FormField label={<label htmlFor="name">Name</label>}>
            <TextInput type="text" name="name" id="name" required />
          </FormField>
          <FormField label={<label htmlFor="nitcGroupId">NITC Group</label>}>
            <Select name="nitcGroupId" id="nitcGroupId">
              <option value="">None</option>
              {sortedGroups.map((g) => (
                <option key={g.id} value={g.id}>
                  {g.id} — {g.nitcType}
                </option>
              ))}
            </Select>
          </FormField>
          <FormField
            label={
              <label htmlFor="nitcParticipantType">NITC Participant Type</label>
            }
          >
            <Select name="nitcParticipantType" id="nitcParticipantType">
              <option value="">None</option>
              {[...data.ses_participant_types].sort().map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </Select>
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

import { useState } from "react";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import type { SettingsActivityDisplayQuery } from "./__generated__/SettingsActivityDisplayQuery.graphql";
import type { SettingsActivityDisplayMutation } from "./__generated__/SettingsActivityDisplayMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import { Button } from "../../components/ui/Button";

export default function SettingsActivityDisplay() {
  const data = useLazyLoadQuery<SettingsActivityDisplayQuery>(
    graphql`
      query SettingsActivityDisplayQuery {
        user {
          disaggregateVirtualPeriods
        }
      }
    `,
    {},
    { fetchPolicy: "store-and-network" },
  );

  const [commitMutation, isMutationInFlight] =
    useMutation<SettingsActivityDisplayMutation>(graphql`
      mutation SettingsActivityDisplayMutation($value: Boolean!) {
        updateMyDisaggregateVirtualPeriods(value: $value) {
          disaggregateVirtualPeriods
        }
      }
    `);

  const { notifyError, notifySuccess } = useNotify();
  const user = data.user;
  const [disaggregate, setDisaggregate] = useState(
    user.disaggregateVirtualPeriods,
  );

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    try {
      await new Promise<void>((resolve, reject) => {
        commitMutation({
          variables: { value: disaggregate },
          onCompleted: () => resolve(),
          onError: reject,
          updater: (store) => {
            store.invalidateStore();
          },
        });
      });
      notifySuccess("Activity display settings saved");
    } catch (err) {
      notifyError(err, "Couldn't save activity display settings");
    }
  }

  return (
    <>
      <h2>Activity display</h2>
      <p>
        Choose how virtual-category activity is shown throughout the admin
        dashboard and Activity tab.
      </p>
      <form onSubmit={handleSubmit}>
        <FieldList>
          <FormField label="Disaggregate virtual periods">
            <div>
              <input
                type="checkbox"
                id="disaggregate-virtual-periods"
                checked={disaggregate}
                onChange={(e) => setDisaggregate(e.target.checked)}
              />
              &nbsp;
              <label htmlFor="disaggregate-virtual-periods">
                Split stats and breakdowns into virtual vs. non-virtual
                categories
              </label>
            </div>
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

import { useState } from "react";
import { graphql, useMutation } from "react-relay";
import type { SettingsDailyEmailQuery } from "./__generated__/SettingsDailyEmailQuery.graphql";
import type { SettingsDailyEmailMutation } from "./__generated__/SettingsDailyEmailMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import { Button } from "../../components/ui/Button";
import MultiSelectList from "../../components/ui/MultiSelectList";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";

export default function SettingsDailyEmail() {
  const data = useRetryableLazyLoadQuery<SettingsDailyEmailQuery>(
    graphql`
      query SettingsDailyEmailQuery @throwOnFieldError {
        user {
          id
          emailSummaryLocationIds
          locations {
            id
            name
          }
        }
      }
    `,
    {},
    { fetchPolicy: "store-and-network" },
  );

  const [commitMutation, isMutationInFlight] =
    useMutation<SettingsDailyEmailMutation>(graphql`
      mutation SettingsDailyEmailMutation($dailyLocationIds: [String!]!) {
        updateMyEmailConfig(dailyLocationIds: $dailyLocationIds) {
          id
          emailSummaryLocationIds
        }
      }
    `);

  const { notifyError, notifySuccess } = useNotify();
  const user = data.user;
  const [selectedLocations, setSelectedLocations] = useState<
    ReadonlySet<string>
  >(() => new Set(user.emailSummaryLocationIds));

  const locations = [...user.locations]
    .sort((a, b) => a.name.localeCompare(b.name))
    .map((location) => ({ id: location.id, name: location.name }));

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    try {
      await new Promise<void>((resolve, reject) => {
        commitMutation({
          variables: { dailyLocationIds: Array.from(selectedLocations) },
          onCompleted: () => resolve(),
          onError: reject,
          updater: (store) => {
            store.invalidateStore();
          },
        });
      });
      notifySuccess("Daily email settings saved");
    } catch (err) {
      notifyError(err, "Couldn't save daily email settings");
    }
  }

  return (
    <>
      <h2>Daily email summary</h2>
      <p>
        Choose which locations to include in your nightly activity summary
        email. Emails are sent just after midnight with the previous day&apos;s
        activity.
      </p>
      <form onSubmit={handleSubmit}>
        <FieldList>
          <FormField label="Daily email — locations">
            <MultiSelectList
              options={locations}
              value={selectedLocations}
              onChange={setSelectedLocations}
              itemLabel="locations"
              emptyMessage="No locations available to your account."
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

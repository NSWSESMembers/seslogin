import { useState } from "react";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import type { SettingsDailyEmailQuery } from "./__generated__/SettingsDailyEmailQuery.graphql";
import type { SettingsDailyEmailMutation } from "./__generated__/SettingsDailyEmailMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import { Button } from "../../components/ui/Button";

export default function SettingsDailyEmail() {
  const data = useLazyLoadQuery<SettingsDailyEmailQuery>(
    graphql`
      query SettingsDailyEmailQuery {
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
  const [selectedLocations, setSelectedLocations] = useState(
    () => new Set(user.emailSummaryLocationIds),
  );

  const locations = [...user.locations].sort((a, b) =>
    a.name.localeCompare(b.name),
  );

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
            {locations.length === 0 && (
              <p>No locations available to your account.</p>
            )}
            {locations.map((loc) => (
              <div key={loc.id}>
                <input
                  type="checkbox"
                  id={`loc-${loc.id}`}
                  checked={selectedLocations.has(loc.id)}
                  onChange={(e) =>
                    setSelectedLocations((prev) => {
                      const next = new Set(prev);
                      if (e.target.checked) next.add(loc.id);
                      else next.delete(loc.id);
                      return next;
                    })
                  }
                />
                &nbsp;
                <label htmlFor={`loc-${loc.id}`}>{loc.name}</label>
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

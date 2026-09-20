import { graphql, useMutation } from "react-relay";
import type { SettingsDailyEmailQuery } from "./__generated__/SettingsDailyEmailQuery.graphql";
import type { SettingsDailyEmailMutation } from "./__generated__/SettingsDailyEmailMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import { Button } from "../../components/ui/Button";
import MultiCombobox from "../../components/ui/MultiCombobox";
import { SectionHeading } from "../../components/ui/SectionHeading";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";

/**
 * Below this many locations the checkbox list is faster than a filter box:
 * every option is on screen at once and each is one click, with nothing to
 * type.
 */
const CHECKBOX_LIMIT = 10;

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

  const locations = [...user.locations].sort((a, b) =>
    a.name.localeCompare(b.name),
  );
  const useCombobox = locations.length > CHECKBOX_LIMIT;

  async function handleSubmit(formData: FormData) {
    const dailyLocationIds = formData
      .getAll("dailyLocations")
      .map((v) => v.toString());
    try {
      await new Promise<void>((resolve, reject) => {
        commitMutation({
          variables: { dailyLocationIds },
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
      <SectionHeading>Daily email summary</SectionHeading>
      <p className="my-4">
        Choose which locations to include in your nightly activity summary
        email. Emails are sent just after midnight with the previous day&apos;s
        activity.
      </p>
      <form action={handleSubmit}>
        <FieldList>
          <FormField
            label={
              useCombobox ? (
                <label htmlFor="dailyLocations">Daily email — locations</label>
              ) : (
                "Daily email — locations"
              )
            }
          >
            {locations.length === 0 && (
              <p className="my-4">No locations available to your account.</p>
            )}
            {useCombobox ? (
              <MultiCombobox
                id="dailyLocations"
                name="dailyLocations"
                options={locations.map((loc) => ({
                  value: loc.id,
                  label: loc.name,
                }))}
                defaultValue={user.emailSummaryLocationIds}
                placeholder="Search locations…"
                emptyText="No locations match"
              />
            ) : (
              locations.map((loc) => (
                <div key={loc.id}>
                  <input
                    type="checkbox"
                    name="dailyLocations"
                    id={`loc-${loc.id}`}
                    value={loc.id}
                    defaultChecked={user.emailSummaryLocationIds.includes(
                      loc.id,
                    )}
                  />
                  &nbsp;
                  <label htmlFor={`loc-${loc.id}`}>{loc.name}</label>
                </div>
              ))
            )}
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

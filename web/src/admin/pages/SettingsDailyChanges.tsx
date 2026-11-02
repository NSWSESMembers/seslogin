import { graphql, useMutation } from "react-relay";
import type { SettingsDailyChangesQuery } from "./__generated__/SettingsDailyChangesQuery.graphql";
import type { SettingsDailyChangesMutation } from "./__generated__/SettingsDailyChangesMutation.graphql";
import { useNotify } from "../components/useNotify";
import { FieldList, FormField } from "../../components/ui/FormField";
import { Button } from "../../components/ui/Button";
import MultiCombobox from "../../components/ui/MultiCombobox";
import { SectionHeading } from "../../components/ui/SectionHeading";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";

/**
 * Below this many locations the checkbox list is faster than a filter box:
 * every option is on screen at once and each is one click, with nothing to
 * type. Matches SettingsDailyEmail's threshold.
 */
const CHECKBOX_LIMIT = 10;

export default function SettingsDailyChanges() {
  const data = useRetryableLazyLoadQuery<SettingsDailyChangesQuery>(
    graphql`
      query SettingsDailyChangesQuery @throwOnFieldError {
        user {
          id
          emailSummaryLocationIds
          dailyChangesLocationIds
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
    useMutation<SettingsDailyChangesMutation>(graphql`
      mutation SettingsDailyChangesMutation(
        $dailyLocationIds: [String!]!
        $dailyChangesLocationIds: [String!]!
      ) {
        updateMyEmailConfig(
          dailyLocationIds: $dailyLocationIds
          dailyChangesLocationIds: $dailyChangesLocationIds
        ) {
          id
          dailyChangesLocationIds
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
    const dailyChangesLocationIds = formData
      .getAll("dailyChangesLocations")
      .map((v) => v.toString());
    try {
      await new Promise<void>((resolve, reject) => {
        commitMutation({
          // This mutation also backs the daily-email settings page. Its
          // dailyLocationIds argument is always fully replaced, so it must
          // be passed through unchanged here — it's the weekly-badge
          // argument (omitted here, so kept) that's optional-and-preserved
          // on omission, not this one. See update_my_email_config's own
          // comment in mutations.rs.
          variables: {
            dailyLocationIds: user.emailSummaryLocationIds,
            dailyChangesLocationIds,
          },
          onCompleted: () => resolve(),
          onError: reject,
          updater: (store) => {
            store.invalidateStore();
          },
        });
      });
      notifySuccess("Daily membership changes settings saved");
    } catch (err) {
      notifyError(err, "Couldn't save daily membership changes settings");
    }
  }

  return (
    <>
      <SectionHeading>Daily membership changes</SectionHeading>
      <p className="my-4">
        A morning email listing members who joined, left or changed at a unit
        yesterday. Choose which locations to include. It is only sent when
        something changed.
      </p>
      <form action={handleSubmit}>
        <FieldList>
          <FormField
            label={
              useCombobox ? (
                <label htmlFor="dailyChangesLocations">
                  Daily membership changes — locations
                </label>
              ) : (
                "Daily membership changes — locations"
              )
            }
          >
            {locations.length === 0 && (
              <p className="my-4">No locations available to your account.</p>
            )}
            {useCombobox ? (
              <MultiCombobox
                id="dailyChangesLocations"
                name="dailyChangesLocations"
                options={locations.map((loc) => ({
                  value: loc.id,
                  label: loc.name,
                }))}
                defaultValue={user.dailyChangesLocationIds}
                placeholder="Search locations…"
                emptyText="No locations match"
              />
            ) : (
              locations.map((loc) => (
                <div key={loc.id}>
                  <input
                    type="checkbox"
                    name="dailyChangesLocations"
                    id={`loc-${loc.id}`}
                    value={loc.id}
                    defaultChecked={user.dailyChangesLocationIds.includes(
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

import { graphql, useMutation } from "react-relay";
import type { SettingsWeeklyBadgeDigestQuery } from "./__generated__/SettingsWeeklyBadgeDigestQuery.graphql";
import type { SettingsWeeklyBadgeDigestMutation } from "./__generated__/SettingsWeeklyBadgeDigestMutation.graphql";
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

export default function SettingsWeeklyBadgeDigest() {
  const data = useRetryableLazyLoadQuery<SettingsWeeklyBadgeDigestQuery>(
    graphql`
      query SettingsWeeklyBadgeDigestQuery @throwOnFieldError {
        user {
          id
          emailSummaryLocationIds
          badgeWeeklyDigestLocationIds
          locations {
            id
            name
            gamificationEnabled
          }
        }
      }
    `,
    {},
    { fetchPolicy: "store-and-network" },
  );

  const [commitMutation, isMutationInFlight] =
    useMutation<SettingsWeeklyBadgeDigestMutation>(graphql`
      mutation SettingsWeeklyBadgeDigestMutation(
        $dailyLocationIds: [String!]!
        $weeklyBadgeLocationIds: [String!]!
      ) {
        updateMyEmailConfig(
          dailyLocationIds: $dailyLocationIds
          weeklyBadgeLocationIds: $weeklyBadgeLocationIds
        ) {
          id
          badgeWeeklyDigestLocationIds
        }
      }
    `);

  const { notifyError, notifySuccess } = useNotify();
  const user = data.user;

  // Only locations with gamification enabled earn badges, so only those are
  // worth offering a digest for.
  const locations = [...user.locations]
    .filter((loc) => loc.gamificationEnabled)
    .sort((a, b) => a.name.localeCompare(b.name));
  const useCombobox = locations.length > CHECKBOX_LIMIT;

  async function handleSubmit(formData: FormData) {
    const weeklyBadgeLocationIds = formData
      .getAll("weeklyBadgeLocations")
      .map((v) => v.toString());
    try {
      await new Promise<void>((resolve, reject) => {
        commitMutation({
          // This mutation also backs the daily-email settings page. Its
          // dailyLocationIds argument is always fully replaced, so it must
          // be passed through unchanged here — it's the weekly-badge
          // argument that's optional-and-preserved on omission, not this
          // one. See update_my_email_config's own comment in mutations.rs.
          variables: {
            dailyLocationIds: user.emailSummaryLocationIds,
            weeklyBadgeLocationIds,
          },
          onCompleted: () => resolve(),
          onError: reject,
          updater: (store) => {
            store.invalidateStore();
          },
        });
      });
      notifySuccess("Weekly badge digest settings saved");
    } catch (err) {
      notifyError(err, "Couldn't save weekly badge digest settings");
    }
  }

  return (
    <>
      <SectionHeading>Weekly badge digest</SectionHeading>
      <p className="my-4">
        Choose which locations to include in your weekly badge digest email.
        Emails are sent weekly, summarising badges earned over the previous 7
        days. Only locations with gamification enabled are shown.
      </p>
      <form action={handleSubmit}>
        <FieldList>
          <FormField
            label={
              useCombobox ? (
                <label htmlFor="weeklyBadgeLocations">
                  Weekly badge digest — locations
                </label>
              ) : (
                "Weekly badge digest — locations"
              )
            }
          >
            {locations.length === 0 && (
              <p className="my-4">
                No gamification-enabled locations available to your account.
              </p>
            )}
            {useCombobox ? (
              <MultiCombobox
                id="weeklyBadgeLocations"
                name="weeklyBadgeLocations"
                options={locations.map((loc) => ({
                  value: loc.id,
                  label: loc.name,
                }))}
                defaultValue={user.badgeWeeklyDigestLocationIds}
                placeholder="Search locations…"
                emptyText="No locations match"
              />
            ) : (
              locations.map((loc) => (
                <div key={loc.id}>
                  <input
                    type="checkbox"
                    name="weeklyBadgeLocations"
                    id={`loc-${loc.id}`}
                    value={loc.id}
                    defaultChecked={user.badgeWeeklyDigestLocationIds.includes(
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

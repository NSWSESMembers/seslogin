import { useState } from "react";
import { graphql, useLazyLoadQuery, useMutation } from "react-relay";
import type { SettingsWeeklyBadgeDigestQuery } from "./__generated__/SettingsWeeklyBadgeDigestQuery.graphql";
import type { SettingsWeeklyBadgeDigestMutation } from "./__generated__/SettingsWeeklyBadgeDigestMutation.graphql";
import { useNotify } from "../components/useNotify";

export default function SettingsWeeklyBadgeDigest() {
  const data = useLazyLoadQuery<SettingsWeeklyBadgeDigestQuery>(
    graphql`
      query SettingsWeeklyBadgeDigestQuery {
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
          emailSummaryLocationIds
          badgeWeeklyDigestLocationIds
        }
      }
    `);

  const { notifyError, notifySuccess } = useNotify();
  const user = data.user;
  const [selectedLocations, setSelectedLocations] = useState(
    () => new Set(user.badgeWeeklyDigestLocationIds),
  );

  const locations = [...user.locations]
    .filter((loc) => loc.gamificationEnabled)
    .sort((a, b) => a.name.localeCompare(b.name));

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    try {
      await new Promise<void>((resolve, reject) => {
        commitMutation({
          variables: {
            dailyLocationIds: user.emailSummaryLocationIds,
            weeklyBadgeLocationIds: Array.from(selectedLocations),
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
      <h2>Weekly badge digest</h2>
      <p>
        Choose which locations to include in your weekly badge digest email.
        Emails are sent weekly, summarising badges earned over the previous 7
        days. Only locations with gamification enabled are shown.
      </p>
      <form onSubmit={handleSubmit}>
        <dl>
          <dt>Weekly badge digest — locations</dt>
          <dd>
            {locations.length === 0 && (
              <p>
                No gamification-enabled locations available to your account.
              </p>
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

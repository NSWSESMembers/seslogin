import { useState } from "react";
import { graphql, useLazyLoadQuery } from "react-relay";
import type { SignOutSessionQuery } from "./__generated__/SignOutSessionQuery.graphql";
import PeriodRow from "./PeriodRow";

interface SignOutSessionProps {
  personId: string;
}

export default function SignOutSession({ personId }: SignOutSessionProps) {
  const data = useLazyLoadQuery<SignOutSessionQuery>(
    graphql`
      query SignOutSessionQuery($personId: ID!) {
        personSignoutSession(personId: $personId) {
          firstName
          categories {
            id
            name
          }
          openPeriods {
            id
            startTime
            location {
              name
            }
          }
        }
      }
    `,
    { personId },
    { fetchPolicy: "network-only" },
  );

  const [closedIds, setClosedIds] = useState<ReadonlySet<string>>(
    () => new Set(),
  );

  const session = data.personSignoutSession;

  if (!session) {
    return (
      <p className="action-panel__message action-panel__message--error">
        This link isn't valid.
      </p>
    );
  }

  const openPeriods = session.openPeriods.filter(
    (period) => !closedIds.has(period.id),
  );

  return (
    <>
      <h1>Hi {session.firstName}</h1>
      {openPeriods.length === 0 ? (
        <p className="action-panel__intro">
          You have no open sessions. All done!
        </p>
      ) : (
        <>
          <p className="action-panel__intro">
            Pick a session below to sign out of it.
          </p>
          <ul className="signout-list">
            {openPeriods.map((period) => (
              <PeriodRow
                key={period.id}
                personId={personId}
                periodId={period.id}
                locationName={period.location.name}
                startTime={period.startTime}
                categories={session.categories}
                onSignedOut={() =>
                  setClosedIds((prev) => new Set(prev).add(period.id))
                }
              />
            ))}
          </ul>
        </>
      )}
    </>
  );
}

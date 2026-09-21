import { Link, useParams } from "react-router";
import { graphql } from "react-relay";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import useSelectedLocation from "../components/useSelectedLocation";
import type { MembersBadgesQuery } from "./__generated__/MembersBadgesQuery.graphql";
import MemberBadgeProgressPanel from "../components/MemberBadgeProgressPanel";

export default function MembersBadges() {
  const params = useParams();
  const selectedLocation = useSelectedLocation();
  const locationId = selectedLocation.id;

  // badgeProgress is rendered by MemberBadgeProgressPanel, which is outside this
  // query file, so Relay cannot see the field usage here.
  /* eslint-disable relay/unused-fields */
  const data = useRetryableLazyLoadQuery<MembersBadgesQuery>(
    graphql`
      query MembersBadgesQuery($id: ID!, $locationId: ID!) @throwOnFieldError {
        person(id: $id) {
          id
          firstName
          lastName
          memberNumber
          badgeProgress(locationId: $locationId) {
            id
            badgeId
            name
            description
            tier
            source
            earned
            awardedAt
            current
            target
          }
        }
      }
    `,
    { id: params.memberId!, locationId },
  );
  /* eslint-enable relay/unused-fields */

  if (!selectedLocation.gamificationEnabled) {
    return (
      <>
        <p className="my-4">Gamification is disabled for this location.</p>
        <p className="my-4">
          <Link to="/admin/members">Back to members</Link>
        </p>
      </>
    );
  }

  const person = data.person;

  return (
    <>
      <p className="my-4">
        Badge progress for {person.firstName} {person.lastName}
        {person.memberNumber ? ` (SES ID ${person.memberNumber})` : ""}.
      </p>
      <p className="my-4">
        <Link to={`/admin/members/${person.id}`}>Back to member details</Link>
      </p>
      <MemberBadgeProgressPanel badgeProgress={person.badgeProgress} />
    </>
  );
}

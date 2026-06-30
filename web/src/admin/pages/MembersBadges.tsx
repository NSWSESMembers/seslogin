import { Link, useParams } from "react-router";
import { graphql, useLazyLoadQuery } from "react-relay";
import useSelectedLocation from "../components/useSelectedLocation";
import type { MembersBadgesQuery } from "./__generated__/MembersBadgesQuery.graphql";
import MemberBadgeProgressPanel from "../components/MemberBadgeProgressPanel";

// badgeProgress is rendered by MemberBadgeProgressPanel, which is outside this
// query file, so Relay cannot see the field usage here.
/* eslint-disable relay/unused-fields */
const membersBadgesQuery = graphql`
  query MembersBadgesQuery($id: ID!, $locationId: ID!) {
    person(id: $id) {
      id
      firstName
      lastName
      memberNumber
      badgeProgress(locationId: $locationId) {
        id
        name
        description
        tier
        source
        earned
        awardedAt
      }
    }
  }
`;
/* eslint-enable relay/unused-fields */

export default function MembersBadges() {
  const params = useParams();
  const selectedLocation = useSelectedLocation();
  const locationId = selectedLocation.id;

  const data = useLazyLoadQuery<MembersBadgesQuery>(membersBadgesQuery, {
    id: params.memberId!,
    locationId,
  });

  const person = data.person;

  return (
    <>
      <p>
        Badge progress for {person.firstName} {person.lastName}
        {person.memberNumber ? ` (SES ID ${person.memberNumber})` : ""}.
      </p>
      <p>
        <Link to={`/admin/members/${person.id}`}>Back to member details</Link>
      </p>
      <MemberBadgeProgressPanel badgeProgress={person.badgeProgress} />
    </>
  );
}

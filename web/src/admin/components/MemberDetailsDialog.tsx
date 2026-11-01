import { graphql } from "react-relay";
import { TextLink } from "../../components/ui/TextLink";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import type { MemberDetailsDialogQuery } from "./__generated__/MemberDetailsDialogQuery.graphql";
import { Muted } from "../../components/ui/Muted";
import { DetailsDialog, Field, Section } from "./DetailsDialog";
import { NONE } from "./detailsFormat";

function MemberDetails({ personId }: { personId: string }) {
  const { person } = useRetryableLazyLoadQuery<MemberDetailsDialogQuery>(
    graphql`
      query MemberDetailsDialogQuery($id: ID!) @throwOnFieldError {
        person(id: $id) {
          id
          firstName
          lastName
          preferredName
          registrationNumber
          sesApiPersonId
          email
          mobile
          phones {
            number
            types
          }
          capabilityTags {
            id
            name
            description
          }
          units {
            entityId
            entityName
            entityCode
            zone
            membershipType
            ranks
            roles
            positions
            isPrimary
          }
        }
      }
    `,
    { id: personId },
  );

  const name = `${person.firstName} ${person.lastName}`.trim();
  // The mobile is also in `phones`; list only the others under "Other phones".
  const otherPhones = person.phones.filter((p) => p.number !== person.mobile);

  return (
    <>
      <dl className="m-0">
        <Field label="Name">
          {name}
          {person.preferredName && person.preferredName !== person.firstName
            ? ` (prefers ${person.preferredName})`
            : ""}
        </Field>
        <Field label="Member number">{person.registrationNumber ?? NONE}</Field>
        <Field label="SES ID">{person.sesApiPersonId ?? NONE}</Field>
        <Field label="Email">
          {person.email ? (
            <TextLink href={`mailto:${person.email}`}>{person.email}</TextLink>
          ) : (
            NONE
          )}
        </Field>
        <Field label="Mobile">
          {person.mobile ? (
            <TextLink href={`tel:${person.mobile}`}>{person.mobile}</TextLink>
          ) : (
            NONE
          )}
        </Field>
        {otherPhones.length > 0 && (
          <Field label="Other phones">
            <ul className="m-0 list-none p-0">
              {otherPhones.map((p) => (
                <li key={p.number}>
                  <TextLink href={`tel:${p.number}`}>{p.number}</TextLink>
                  {p.types.length > 0 && (
                    <span className="text-ink-muted">
                      {" "}
                      ({p.types.join(", ")})
                    </span>
                  )}
                </li>
              ))}
            </ul>
          </Field>
        )}
      </dl>

      <Section title="Qualifications">
        {person.capabilityTags.length === 0 ? (
          <Muted>None recorded</Muted>
        ) : (
          <ul className="m-0 list-none p-0">
            {person.capabilityTags.map((tag) => (
              <li key={tag.id} className="py-0.5">
                <span title={tag.description ?? undefined}>
                  {tag.name ?? `Tag ${tag.id}`}
                </span>
                {tag.description && (
                  <span className="text-sm text-ink-muted">
                    {" "}
                    — {tag.description}
                  </span>
                )}
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Section title="Units">
        {person.units.length === 0 ? (
          <Muted>None recorded</Muted>
        ) : (
          <ul className="m-0 list-none p-0">
            {person.units.map((unit) => (
              <li
                key={unit.entityId}
                className="border-b border-line-faint py-1.5"
              >
                <div className="font-semibold">
                  {unit.entityName ?? `Entity ${unit.entityId}`}
                  {unit.entityCode && (
                    <span className="font-normal text-ink-muted">
                      {" "}
                      ({unit.entityCode})
                    </span>
                  )}
                  {unit.isPrimary && (
                    <span className="ml-2 rounded-full border border-line px-2 py-px text-xs font-semibold">
                      Primary
                    </span>
                  )}
                </div>
                <dl className="m-0 text-sm">
                  {unit.zone && <Field label="Zone">{unit.zone}</Field>}
                  {unit.membershipType && (
                    <Field label="Membership">{unit.membershipType}</Field>
                  )}
                  {unit.ranks.length > 0 && (
                    <Field label="Ranks">{unit.ranks.join(", ")}</Field>
                  )}
                  {unit.roles.length > 0 && (
                    <Field label="Roles">{unit.roles.join(", ")}</Field>
                  )}
                  {unit.positions.length > 0 && (
                    <Field label="Positions">{unit.positions.join(", ")}</Field>
                  )}
                </dl>
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Muted className="mt-4 text-sm">
        Contact details and units come from the SES directory and are refreshed
        once a day. Qualifications come from member sync.
      </Muted>
    </>
  );
}

export default function MemberDetailsDialog({
  personId,
  personName,
  onClose,
}: {
  personId: string;
  personName: string;
  onClose: () => void;
}) {
  return (
    <DetailsDialog title={personName} onClose={onClose}>
      <MemberDetails personId={personId} />
    </DetailsDialog>
  );
}

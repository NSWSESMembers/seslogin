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
          units {
            entityId
            entityName
            entityCode
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

      <Section title="Units">
        {person.units.length === 0 ? (
          <Muted>None recorded</Muted>
        ) : (
          <ul className="m-0 list-none p-0">
            {person.units.map((unit) => (
              <li key={unit.entityId} className="py-0.5">
                {unit.entityName ?? `Entity ${unit.entityId}`}
                {unit.entityCode && (
                  <span className="text-ink-muted"> ({unit.entityCode})</span>
                )}
                {unit.isPrimary && (
                  <span className="ml-2 rounded-full border border-line px-2 py-px text-xs font-semibold">
                    Primary
                  </span>
                )}
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Muted className="mt-4 text-sm">
        Contact details and units come from the SES directory and are refreshed
        once a day.
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

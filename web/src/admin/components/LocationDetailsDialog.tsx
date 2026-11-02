import { graphql } from "react-relay";
import { TextLink } from "../../components/ui/TextLink";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import type {
  LocationDetailsDialogQuery,
  LocationDetailsDialogQuery$data,
} from "./__generated__/LocationDetailsDialogQuery.graphql";
import { Muted } from "../../components/ui/Muted";
import { DetailsDialog, Field, Section } from "./DetailsDialog";
import { mapUrl, NONE } from "./detailsFormat";

type Address = LocationDetailsDialogQuery$data["location"]["address"];

// The directory's one-line form when it has one, else the parts that are set.
function addressText(address: Address): string | null {
  if (!address) return null;
  if (address.formatted) return address.formatted;
  const locality = [address.suburb, address.state, address.postcode]
    .filter(Boolean)
    .join(" ");
  const text = [address.line1, address.line2, locality]
    .filter(Boolean)
    .join(", ");
  return text || null;
}

function LocationDetails({ locationId }: { locationId: string }) {
  const { location } = useRetryableLazyLoadQuery<LocationDetailsDialogQuery>(
    graphql`
      query LocationDetailsDialogQuery($id: ID!) @throwOnFieldError {
        location(id: $id) {
          name
          sesApiHeadquartersId
          code
          zone {
            name
          }
          latitude
          longitude
          sesDirectoryEntityId
          address {
            line1
            line2
            suburb
            state
            postcode
            formatted
          }
          postalAddress {
            line1
            line2
            suburb
            state
            postcode
            formatted
          }
          email
          phone
          accreditations {
            type
            status
            number
            note
          }
        }
      }
    `,
    { id: locationId },
  );

  const hasCoordinates =
    location.latitude != null && location.longitude != null;

  return (
    <>
      <dl className="m-0">
        <Field label="Name">{location.name}</Field>
        <Field label="SES HQ ID">{location.sesApiHeadquartersId ?? NONE}</Field>
        <Field label="Code">{location.code ?? NONE}</Field>
        <Field label="Zone">{location.zone?.name ?? NONE}</Field>
        <Field label="Coordinates">
          {hasCoordinates ? (
            <TextLink
              href={mapUrl(location.latitude!, location.longitude!)}
              target="_blank"
              rel="noreferrer"
            >
              {location.latitude}, {location.longitude}
            </TextLink>
          ) : (
            NONE
          )}
        </Field>
        <Field label="Directory entity">
          {location.sesDirectoryEntityId ?? NONE}
        </Field>
        <Field label="Address">{addressText(location.address) ?? NONE}</Field>
        <Field label="Postal address">
          {addressText(location.postalAddress) ?? NONE}
        </Field>
        <Field label="Email">
          {location.email ? (
            <TextLink href={`mailto:${location.email}`}>
              {location.email}
            </TextLink>
          ) : (
            NONE
          )}
        </Field>
        <Field label="Phone">
          {location.phone ? (
            <TextLink href={`tel:${location.phone}`}>{location.phone}</TextLink>
          ) : (
            NONE
          )}
        </Field>
      </dl>

      <Section title="Accreditations">
        {location.accreditations.length === 0 ? (
          <Muted>None recorded</Muted>
        ) : (
          <ul className="m-0 list-none p-0">
            {location.accreditations.map((a) => (
              <li
                key={`${a.type}-${a.number ?? ""}`}
                className="border-b border-line-faint py-1.5"
              >
                <span className="font-semibold">{a.type}</span>
                {a.status && <span> — {a.status}</span>}
                {a.number && (
                  <span className="text-ink-muted"> (#{a.number})</span>
                )}
                {a.note && (
                  <div className="text-sm text-ink-muted">{a.note}</div>
                )}
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Muted className="mt-4 text-sm">
        Unit details come from the SES API and the SES directory and are
        refreshed once a day.
      </Muted>
    </>
  );
}

export default function LocationDetailsDialog({
  locationId,
  locationName,
  onClose,
}: {
  locationId: string;
  locationName: string;
  onClose: () => void;
}) {
  return (
    <DetailsDialog title={`${locationName} details`} onClose={onClose}>
      <LocationDetails locationId={locationId} />
    </DetailsDialog>
  );
}

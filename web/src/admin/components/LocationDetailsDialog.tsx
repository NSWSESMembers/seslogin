import { graphql } from "react-relay";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import type { LocationDetailsDialogQuery } from "./__generated__/LocationDetailsDialogQuery.graphql";
import { Muted } from "../../components/ui/Muted";
import { DetailsDialog, Field } from "./DetailsDialog";
import { NONE } from "./detailsFormat";

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
          sesDirectoryEntityId
        }
      }
    `,
    { id: locationId },
  );

  return (
    <>
      <dl className="m-0">
        <Field label="Name">{location.name}</Field>
        <Field label="SES HQ ID">{location.sesApiHeadquartersId ?? NONE}</Field>
        <Field label="Code">{location.code ?? NONE}</Field>
        <Field label="Zone">{location.zone?.name ?? NONE}</Field>
        <Field label="Directory entity">
          {location.sesDirectoryEntityId ?? NONE}
        </Field>
      </dl>

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

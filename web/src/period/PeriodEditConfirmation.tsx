import { formatFullDateTime, formatTimeDiff } from "../lib/time";
import { Button } from "../components/ui/Button";
import { FieldList, FormField } from "../components/ui/FormField";
import {
  Panel,
  PanelBox,
  PanelIntro,
  PanelTitle,
} from "../components/ui/Panel";
import type { SavedEntry } from "./PeriodEditForm";
import { periodEditCopy } from "./copy";

/**
 * Shown after a successful save. The link stays usable for its full 48 hours, so
 * this offers a way back to the form rather than being a dead end — a member who
 * mistypes a time can fix it without asking for a new link.
 */
export default function PeriodEditConfirmation({
  saved,
  onChangeAgain,
}: {
  saved: SavedEntry;
  onChangeAgain: () => void;
}) {
  const start = new Date(saved.startTime * 1000);
  const end = new Date(saved.endTime * 1000);
  // The entry now has both times either way, so the wording has to come from
  // how the member arrived rather than from what was saved.
  const copy = periodEditCopy(saved.wasIncomplete);

  return (
    <Panel>
      <PanelBox>
        <PanelTitle>{copy.confirmationTitle}</PanelTitle>
        <PanelIntro>{copy.confirmationIntro}</PanelIntro>

        <FieldList>
          <FormField label="Location">{saved.locationName}</FormField>
          <FormField label="Activity">{saved.categoryName}</FormField>
          <FormField label="Start time">{formatFullDateTime(start)}</FormField>
          <FormField label="End time">{formatFullDateTime(end)}</FormField>
          <FormField label="Total">{formatTimeDiff(start, end)}</FormField>
        </FieldList>

        <Button onClick={onChangeAgain}>Make another change</Button>
      </PanelBox>
    </Panel>
  );
}

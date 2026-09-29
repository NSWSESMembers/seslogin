import { PanelMessage } from "../../components/ui/Panel";

/** Shown in place of a write form when the user has Read only access. */
export default function ReadOnlyNotice() {
  return (
    <div className="mx-auto max-w-xl text-left">
      <PanelMessage variant="warning">
        You have read-only access to this location.
      </PanelMessage>
    </div>
  );
}

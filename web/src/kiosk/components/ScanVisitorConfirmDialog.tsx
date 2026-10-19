import { useEffect } from "react";
import { Button } from "../../components/ui/Button";
import { Dialog, DialogActions, DialogTitle } from "../../components/ui/Dialog";
import { useSuspendScanFocus } from "../lib/scanFocusLeases";

const VISITOR_CONFIRM_FOCUS_LEASE_ID = "scan:visitor-confirm";

// Someone who walks away without answering would otherwise leave the kiosk
// unable to take the next scan, so an unanswered question counts as "No" —
// nothing has been written, and the member can just scan again.
const AUTO_CLOSE_MS = 30_000;

export type VisitorConfirmation = {
  memberId: string;
  name: string;
  homeUnitName: string;
};

/**
 * Asked before signing in a member whose home unit isn't this kiosk's location
 * (the kiosk's "Confirm visitors" option). Most mistyped member IDs belong to
 * someone at another unit, so this is where they get caught. Nothing has been
 * written yet: "Yes" signs them in, "No" (or no answer) drops the scan.
 */
export default function ScanVisitorConfirmDialog(props: {
  visitor: VisitorConfirmation;
  locationName: string | null;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const { visitor, locationName, onConfirm, onCancel } = props;

  // Only mounted while the question is up, so a scan can't land in the hidden
  // member ID input underneath it.
  useSuspendScanFocus(VISITOR_CONFIRM_FOCUS_LEASE_ID);

  useEffect(() => {
    const timeoutId = window.setTimeout(onCancel, AUTO_CLOSE_MS);
    return () => window.clearTimeout(timeoutId);
  }, [onCancel]);

  return (
    <Dialog onDismiss={onCancel}>
      <DialogTitle>Are you {visitor.name}?</DialogTitle>
      <p className="m-0 text-lg">
        Member ID <span className="font-mono">{visitor.memberId}</span> belongs
        to <strong>{visitor.name}</strong> from{" "}
        <strong>{visitor.homeUnitName}</strong>
        {locationName ? <>, not {locationName}</> : null}.
      </p>
      <p className="m-0 text-ink-muted">
        If that isn't you, press No and check the number you entered.
      </p>
      <DialogActions>
        <Button variant="secondary" size="panel" onClick={onCancel}>
          No
        </Button>
        <Button variant="kiosk" size="panel" onClick={onConfirm}>
          Yes, sign me in
        </Button>
      </DialogActions>
    </Dialog>
  );
}

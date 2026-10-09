import { useEffect, useRef } from "react";
import { Dialog, DialogTitle } from "../../components/ui/Dialog";
import Keypad from "./Keypad";
import { MEMBER_ID_LENGTH } from "../../lib/memberId";

// The typed digits sit on the always-dark display strip, so the box stays
// light-on-dark in both themes — no dark: variants. Its width is fixed to
// MEMBER_ID_LENGTH digits plus the cursor (tabular digits, so every one is the
// same `ch` wide) so the dialog doesn't resize as digits are typed.
const displayWidth = `${MEMBER_ID_LENGTH + 1}ch`;

/**
 * The on-screen keypad for entering a member ID without a keyboard or barcode
 * scanner, opened from the scan screen on a touch kiosk (see interfaceMode).
 *
 * It drives the member ID input rather than holding the typed value itself, so
 * the input stays the single source of truth and a scanner, a physical keyboard
 * and this pad can all be used against the same entry. `value` is that input's
 * current text, shown here because the dialog covers the input itself.
 *
 * The input is not focused while the pad is open — on an iPad a focused input
 * brings up the system keyboard on top of the pad — so a barcode scanner or
 * physical keyboard would otherwise type into nothing. The pad listens for
 * those keys itself and treats them as presses of its own keys.
 */
export default function ScanNumberPadDialog(props: {
  value: string;
  onDigit: (digit: string) => void;
  onDelete: () => void;
  onSubmit: () => void;
  onClose: () => void;
  submitDisabled?: boolean;
}) {
  const { value, onDigit, onDelete, onSubmit, onClose, submitDisabled } = props;

  // Held in a ref so the listener is attached once, not on every keystroke.
  const handlersRef = useRef(props);
  useEffect(() => {
    handlersRef.current = props;
  });

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.ctrlKey || event.metaKey || event.altKey) {
        return;
      }
      const handlers = handlersRef.current;
      if (/^[0-9]$/.test(event.key)) {
        handlers.onDigit(event.key);
      } else if (event.key === "Backspace") {
        handlers.onDelete();
      } else if (event.key === "Enter") {
        // A scanner's trailing newline lands here, so Enter submits whenever
        // the pad's Confirm key would.
        if (!handlers.submitDisabled) {
          handlers.onSubmit();
        }
      } else if (event.key === "Escape") {
        handlers.onClose();
      } else {
        return;
      }
      event.preventDefault();
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, []);

  return (
    <Dialog onDismiss={onClose} width="w-auto">
      <DialogTitle>Enter your SES ID</DialogTitle>
      <div className="flex justify-center rounded-[14px] bg-neutral-800 px-3 py-3.5">
        <div
          role="status"
          aria-label="Member ID"
          style={{ width: displayWidth }}
          className="box-content rounded-lg bg-white px-3 py-1 text-left font-mono text-4xl font-bold text-neutral-800 tabular-nums"
        >
          {value}
          {value.length < MEMBER_ID_LENGTH && (
            <span
              aria-hidden="true"
              className="ml-px inline-block h-[1em] w-0.5 translate-y-[0.15em] animate-pulse bg-accent motion-reduce:animate-none"
            />
          )}
          {"\u200b"}
        </div>
      </div>
      <Keypad
        onDigit={onDigit}
        onDelete={onDelete}
        onClose={onClose}
        onConfirm={onSubmit}
        confirmDisabled={submitDisabled}
      />
    </Dialog>
  );
}

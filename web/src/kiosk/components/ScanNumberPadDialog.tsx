import { Dialog, DialogTitle } from "../../components/ui/Dialog";
import { Button } from "../../components/ui/Button";
import { MEMBER_ID_LENGTH } from "../../lib/memberId";

// Key faces are laid out phone-style (1 top-left), which is what people expect
// from a touchscreen. The bottom row is delete / 0 / enter, so the two actions
// sit either side of the digit rather than being reached for somewhere else.
const DIGIT_KEYS = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];

// Every key is the same box — sized here rather than left to the text inside it,
// so the delete and enter keys line up with the digits.
const keyBase =
  "flex h-20 w-28 cursor-pointer items-center justify-center rounded-[14px] leading-none shadow-md disabled:cursor-default disabled:opacity-30 disabled:shadow-none";
const keyDigit = `${keyBase} bg-neutral-800 text-[2.25em] text-white active:bg-neutral-600 dark:bg-neutral-700 dark:active:bg-neutral-500`;
const keyAux = `${keyBase} bg-neutral-200 text-[1.75em] text-neutral-700 active:bg-neutral-300 dark:bg-neutral-800 dark:text-neutral-300 dark:active:bg-neutral-700`;
const keyConfirm = `${keyBase} bg-confirm text-[1.75em] text-white active:bg-confirm-active disabled:bg-neutral-300 disabled:text-neutral-500 dark:disabled:bg-neutral-700 dark:disabled:text-neutral-500`;
// The digit boxes sit on the always-dark display strip, so they stay
// light-on-dark in both themes — no dark: variants.
const digitBoxBase =
  "box-border w-11 rounded-lg border-4 bg-white text-center text-neutral-800";
// The ring marks the digit the next key press fills. Its border *colour* lives
// only in these two: two border-colour utilities in one class string resolve by
// stylesheet order rather than the order they are written, so a colour in
// digitBoxBase would beat the ring.
const digitBoxCurrent = "border-accent";
const digitBoxIdle = "border-transparent";

/**
 * One key. Pressing it must not move focus: the member ID input behind the
 * dialog keeps it, so a barcode scan or a keyboard is still typed into the same
 * entry while the pad is open, and a key that took focus would also start the
 * input's refocus timer on every tap. Preventing the default on mousedown keeps
 * focus where it is, and `tabIndex={-1}` keeps the pad out of the tab order.
 */
function Key(props: {
  className: string;
  label: string;
  onPress: () => void;
  face?: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      tabIndex={-1}
      aria-label={props.label}
      disabled={props.disabled}
      className={props.className}
      onMouseDown={(event) => event.preventDefault()}
      onClick={props.onPress}
    >
      {props.face ?? props.label}
    </button>
  );
}

/**
 * The on-screen keypad for entering a member ID without a keyboard or barcode
 * scanner, opened from the scan screen when the session config enables it.
 *
 * It drives the member ID input rather than holding the typed value itself, so
 * the input stays the single source of truth and a scanner, a physical keyboard
 * and this pad can all be used against the same entry. `value` is that input's
 * current text, shown here because the dialog covers the input itself.
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

  return (
    <Dialog onDismiss={onClose} width="w-auto">
      <DialogTitle>Enter your SES ID</DialogTitle>
      <div className="flex justify-center gap-1.5 rounded-[14px] bg-neutral-800 px-3 py-3.5 text-[2em] font-bold">
        {Array.from({ length: MEMBER_ID_LENGTH }, (_, index) => (
          <span
            key={index}
            className={`${digitBoxBase} ${index === value.length ? digitBoxCurrent : digitBoxIdle}`}
          >
            {value.charAt(index) || "\xa0"}
          </span>
        ))}
      </div>
      <div className="grid grid-cols-3 gap-2.5">
        {DIGIT_KEYS.map((digit) => (
          <Key
            key={digit}
            className={keyDigit}
            label={digit}
            onPress={() => onDigit(digit)}
          />
        ))}
        <Key className={keyAux} label="Delete" face="⌫" onPress={onDelete} />
        <Key className={keyDigit} label="0" onPress={() => onDigit("0")} />
        <Key
          className={keyConfirm}
          label="Enter"
          face="⏎"
          onPress={onSubmit}
          disabled={submitDisabled}
        />
      </div>
      <Button variant="kiosk" size="panel" type="button" onClick={onClose}>
        Close
      </Button>
    </Dialog>
  );
}

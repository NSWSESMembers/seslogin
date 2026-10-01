// Key faces are laid out phone-style (1 top-left), which is what people expect
// from a touchscreen. The bottom row is close / 0 / delete, with a full-width
// confirm bar underneath, so the actions sit beside the digits rather than being
// reached for somewhere else.
const DIGIT_KEYS = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];

// Every key is the same box — sized here rather than left to the text inside it,
// so the close and delete keys line up with the digits.
const keyBase =
  "flex h-24 w-32 cursor-pointer items-center justify-center rounded-[14px] leading-none shadow-md disabled:cursor-default disabled:opacity-30 disabled:shadow-none";
const keyDigit = `${keyBase} bg-neutral-800 text-5xl text-white active:bg-neutral-600 dark:bg-neutral-700 dark:active:bg-neutral-500`;
const keyAux = `${keyBase} bg-neutral-200 text-4xl text-neutral-700 active:bg-neutral-300 dark:bg-neutral-800 dark:text-neutral-300 dark:active:bg-neutral-700`;
const keyConfirm = `${keyBase} col-span-3 w-full bg-confirm text-4xl text-white active:bg-confirm-active disabled:bg-neutral-300 disabled:text-neutral-500 dark:disabled:bg-neutral-700 dark:disabled:text-neutral-500`;

/**
 * One key. Pressing it must not take focus, so the owning dialog's own keyboard
 * handling keeps working for a barcode scan or a keyboard after a tap.
 * Preventing the default on mousedown keeps focus where it is, and
 * `tabIndex={-1}` keeps the pad out of the tab order.
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
 * The on-screen numeric keypad shared by the kiosk's member ID and date/time
 * entry: digits 1-9, a close / 0 / delete row, and a Confirm bar. It holds no
 * state — the caller owns the value and handles each press.
 */
export default function Keypad(props: {
  onDigit: (digit: string) => void;
  onDelete: () => void;
  onClose: () => void;
  onConfirm: () => void;
  confirmDisabled?: boolean;
}) {
  return (
    <div className="grid grid-cols-3 gap-2.5">
      {DIGIT_KEYS.map((digit) => (
        <Key
          key={digit}
          className={keyDigit}
          label={digit}
          onPress={() => props.onDigit(digit)}
        />
      ))}
      <Key className={keyAux} label="Close" face="×" onPress={props.onClose} />
      <Key className={keyDigit} label="0" onPress={() => props.onDigit("0")} />
      <Key
        className={keyAux}
        label="Delete"
        face="⌫"
        onPress={props.onDelete}
      />
      <Key
        className={keyConfirm}
        label="Confirm"
        onPress={props.onConfirm}
        disabled={props.confirmDisabled}
      />
    </div>
  );
}

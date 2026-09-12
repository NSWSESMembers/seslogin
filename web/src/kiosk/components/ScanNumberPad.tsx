// Key faces are laid out phone-style (1 top-left), which is what people expect
// from a touchscreen. The bottom row is delete / 0 / enter, so the two actions
// sit either side of the digit rather than being reached for somewhere else.
const DIGIT_KEYS = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];

// Every key is the same box — sized here rather than left to the text inside it,
// so the delete and enter keys line up with the digits instead of shrinking to
// their smaller glyphs.
const keyBase =
  "flex h-22 w-32 cursor-pointer items-center justify-center rounded-[14px] leading-none shadow-md disabled:cursor-default disabled:opacity-30 disabled:shadow-none";
const keyDigit = `${keyBase} bg-neutral-800 text-[2.5em] text-white active:bg-neutral-600 dark:bg-neutral-700 dark:active:bg-neutral-500`;
const keyAux = `${keyBase} bg-neutral-200 text-[2em] text-neutral-700 active:bg-neutral-300 dark:bg-neutral-800 dark:text-neutral-300 dark:active:bg-neutral-700`;

/**
 * One key. Pressing it must not move focus: the member ID input holds focus so
 * that a barcode scan is never typed into nothing, and a keypad button that took
 * it would also start the input's refocus timer on every tap. Preventing the
 * default on mousedown keeps focus where it is, and `tabIndex={-1}` keeps the pad
 * out of the tab order — anyone on a keyboard types into the input directly.
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
 * scanner. Shown on the main scan screen when the session config enables it.
 *
 * It drives the member ID input rather than holding the typed value itself, so
 * the input stays the single source of truth and a scanner, a physical keyboard
 * and this pad can all be used against the same half-typed ID.
 */
export default function ScanNumberPad(props: {
  onDigit: (digit: string) => void;
  onDelete: () => void;
  onSubmit: () => void;
  submitDisabled?: boolean;
}) {
  const { onDigit, onDelete, onSubmit, submitDisabled } = props;

  return (
    <div
      className="mx-auto mt-6 grid w-fit grid-cols-3 gap-2.5"
      role="group"
      aria-label="Number pad"
    >
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
        className={keyAux}
        label="Enter"
        face="⏎"
        onPress={onSubmit}
        disabled={submitDisabled}
      />
    </div>
  );
}

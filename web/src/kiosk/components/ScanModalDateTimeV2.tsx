import { useEffect, useState } from "react";
import { formatDayDate, isSameDay } from "../../lib/time";
import { Dialog } from "../../components/ui/Dialog";

type AmPm = "AM" | "PM";

// The four time digits, HHMM. `null` is a digit that hasn't been entered yet —
// holes are possible in the middle, since the caret can be moved anywhere and a
// digit deleted in place.
type Digits = [string | null, string | null, string | null, string | null];

// Index of the digit the orange ring sits on. There is always exactly one: the
// ring wraps from the last digit back to the first rather than going away, so
// the field is never in a state where pressing a number has no visible target.
type Caret = 0 | 1 | 2 | 3;

function toDigits(value: string): Digits {
  return [0, 1, 2, 3].map((i) =>
    i < value.length ? value.charAt(i) : null,
  ) as Digits;
}

function isComplete(digits: Digits): boolean {
  return digits.every((d) => d !== null);
}

// Where the ring starts: on the first blank digit, or on the first digit when
// the field opens already full (which it normally does — it prefills with the
// current time), so it is visible up front that pressing a number overwrites
// the leading digit.
function initialCaret(digits: Digits): Caret {
  const first = digits.findIndex((d) => d === null);
  return first === -1 ? 0 : (first as Caret);
}

// Whether `key` is allowed as the digit at `index`, given the digits around it.
// Hours run 00-23 and minutes 00-59, and the hour rule cuts both ways now that
// digits can be edited out of order: typing a 2 into the hour tens is only legal
// if the hour units is 3 or less (or still blank).
function isValidDigit(digits: Digits, index: number, key: number): boolean {
  if (index === 0) {
    if (key > 2) return false;
    if (key === 2 && digits[1] !== null && Number(digits[1]) > 3) return false;
    return true;
  }
  if (index === 1) {
    return !(digits[0] === "2" && key > 3);
  }
  if (index === 2) {
    return key <= 5;
  }
  return true;
}

function dateOnly(d: Date): Date {
  const result = new Date(d);
  result.setHours(0, 0, 0, 0);
  return result;
}

function yesterday(): Date {
  const d = new Date();
  d.setDate(d.getDate() - 1);
  return dateOnly(d);
}

function to12HourDigits(hours24: number, minutes: number): string {
  let hour12 = hours24 % 12;
  if (hour12 === 0) {
    hour12 = 12;
  }
  return String(hour12).padStart(2, "0") + String(minutes).padStart(2, "0");
}

const dateBtnClasses =
  "shrink-0 rounded-xl bg-neutral-800 px-4 py-2.5 text-2xl text-white shadow-md disabled:cursor-default disabled:opacity-30 dark:bg-neutral-700";
const dateChipBase = "flex-1 rounded-[10px] border-2 p-2.5 text-xl shadow-sm";
const dateChipOff =
  "border-neutral-300 bg-white text-neutral-700 dark:border-line dark:bg-surface-raised dark:text-ink";
const dateChipSelected = "border-accent bg-accent text-white";
// digitBoxBase / ampmMini* sit on the always-dark time "screen" (the neutral-800
// <th> below), so they stay light-on-dark in both themes — no dark: variants.
const digitBoxBase =
  "mx-1 box-border w-[52px] cursor-pointer rounded-[10px] border-4 bg-white text-center text-neutral-800";
// The orange caret ring, always on exactly one digit. The border *colour* lives
// entirely in these two, never in digitBoxBase — two border-colour utilities in
// one class string resolve by stylesheet order rather than by the order they are
// written, so the ring would lose to the transparent default.
const digitBoxCurrent = "border-accent";
const digitBoxIdle = "border-transparent hover:border-accent-light";
const ampmMiniBase = "rounded-lg border-2 px-3 py-[3px] text-base shadow-none";
const ampmMiniOff = "border-neutral-300 bg-white text-neutral-700";
const ampmMiniSelected = "border-accent bg-accent text-white";
const keyDigitBtn =
  "block w-40 cursor-pointer rounded-[14px] bg-neutral-800 px-2.5 py-[18px] text-[64px] text-white no-underline shadow-md active:bg-neutral-600 dark:bg-neutral-700 dark:active:bg-neutral-500";
const keyAuxBtn =
  "block w-40 cursor-pointer rounded-[14px] bg-neutral-200 px-2.5 py-[18px] text-[32px] text-neutral-700 no-underline shadow-md active:bg-neutral-300 dark:bg-neutral-800 dark:text-neutral-300 dark:active:bg-neutral-700";
const keyConfirmBtn =
  "block w-full cursor-pointer rounded-[14px] bg-[#2f7d4f] px-2.5 py-[18px] text-[40px] text-white no-underline shadow-md active:bg-[#276a43] disabled:cursor-default disabled:bg-neutral-300 disabled:text-neutral-500 disabled:shadow-none dark:disabled:bg-neutral-700 dark:disabled:text-neutral-500";

export function Inner(props: {
  onSave: (field: string, date: Date, value: string) => void;
  onClose: () => void;
  field: string;
  initialDate: Date;
  initialAmPm: AmPm;
  initialValue: string;
}) {
  const [digits, setDigits] = useState<Digits>(() =>
    toDigits(props.initialValue),
  );
  const [caret, setCaret] = useState<Caret>(() =>
    initialCaret(toDigits(props.initialValue)),
  );
  const [ampm, setAmpm] = useState<AmPm>(props.initialAmPm);
  const [date, setDate] = useState<Date>(props.initialDate);

  const isToday = isSameDay(date, new Date());
  const isYesterday = isSameDay(date, yesterday());
  const complete = isComplete(digits);

  // "00" or "13"-"23" can only mean 24-hour time; "01"-"12" is ambiguous and
  // needs the AM/PM toggle to resolve
  const hourEntered =
    digits[0] !== null && digits[1] !== null
      ? Number(digits[0] + digits[1])
      : null;
  const isUnambiguous24Hour =
    hourEntered !== null && (hourEntered === 0 || hourEntered >= 13);

  function button(key: string) {
    if (key === "DEL") {
      del();
      return;
    }
    const digit = Number(key);
    // A digit is always replaced in place — entering one never disturbs the
    // others.
    if (!isValidDigit(digits, caret, digit)) return;
    const next = [...digits] as Digits;
    next[caret] = key;
    setDigits(next);
    // Move right, wrapping past the last digit back to the first so a second
    // pass over the same field can start straight away.
    setCaret(caret === 3 ? 0 : ((caret + 1) as Caret));
  }

  // Backspace: clear the digit under the ring if there is one, otherwise the one
  // to its left.
  function del() {
    const target = digits[caret] !== null ? caret : caret - 1;
    if (target < 0) return;
    const next = [...digits] as Digits;
    next[target] = null;
    setDigits(next);
    setCaret(target as Caret);
  }

  // Arrow keys clamp rather than wrap: unlike entering a digit, moving the ring
  // is a deliberate aim at one box, and wrapping off an end would overshoot it.
  function moveCaret(delta: -1 | 1) {
    setCaret(Math.min(3, Math.max(0, caret + delta)) as Caret);
  }

  function char(index: number): string {
    return digits[index] ?? "\xa0"; // non-breaking space
  }

  function current(index: number): boolean {
    return index === caret;
  }

  // Tapping a digit parks the ring on it, so the next key press replaces it.
  // Kept out of the tab order and denied focus on press: the keypad and the
  // window-level key handler below are the input surface, and a focused digit
  // would swallow Enter.
  function digitBox(index: number) {
    return (
      <button
        type="button"
        tabIndex={-1}
        aria-label={`${index < 2 ? "Hour" : "Minute"} digit ${(index % 2) + 1}`}
        className={`${digitBoxBase} ${current(index) ? digitBoxCurrent : digitBoxIdle}`}
        onMouseDown={(event) => event.preventDefault()}
        onClick={() => setCaret(index as Caret)}
      >
        {char(index)}
      </button>
    );
  }

  // Not scoped to a ref/focus: the modal owns the screen while it is open, and
  // the on-screen keypad is the primary input, so nothing else should be
  // listening. No dependency array — every render re-registers a handler closed
  // over the current digits, caret and date.
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      let handled = true;
      if (event.key.length === 1 && event.key >= "0" && event.key <= "9") {
        button(event.key);
      } else if (event.key === "Backspace" || event.key === "Delete") {
        del();
      } else if (event.key === "ArrowLeft") {
        moveCaret(-1);
      } else if (event.key === "ArrowRight") {
        moveCaret(1);
      } else if (event.key === "Enter") {
        if (complete) confirm();
      } else if (event.key === "Escape") {
        props.onClose();
      } else if (
        !isUnambiguous24Hour &&
        (event.key === "a" || event.key === "A")
      ) {
        setAmpm("AM");
      } else if (
        !isUnambiguous24Hour &&
        (event.key === "p" || event.key === "P")
      ) {
        setAmpm("PM");
      } else {
        handled = false;
      }
      // Swallowing Enter/Space-alikes also stops a keypad button that happens to
      // have focus from firing a second time.
      if (handled) event.preventDefault();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  function changeDay(delta: number) {
    const next = new Date(date);
    next.setDate(next.getDate() + delta);
    if (dateOnly(next) > dateOnly(new Date())) {
      return;
    }
    setDate(dateOnly(next));
  }

  function confirm() {
    const hourTyped = Number(`${digits[0]}${digits[1]}`);
    const minutes = Number(`${digits[2]}${digits[3]}`);
    const hour24 =
      hourTyped === 0 || hourTyped >= 13
        ? hourTyped
        : ampm === "AM"
          ? hourTyped % 12
          : (hourTyped % 12) + 12;
    const paddedValue =
      String(hour24).padStart(2, "0") + String(minutes).padStart(2, "0");
    props.onSave(props.field, date, paddedValue);
  }

  return (
    <Dialog onDismiss={props.onClose} width="w-auto">
      <div>
        <div className="mb-3.5 w-full">
          <div className="mb-2 flex items-center gap-2">
            <button className={dateBtnClasses} onClick={() => changeDay(-1)}>
              &#8592;
            </button>
            <span className="flex-1 text-center text-[26px] font-bold text-neutral-800 dark:text-neutral-100">
              {formatDayDate(date)}
            </span>
            <button
              className={dateBtnClasses}
              onClick={() => changeDay(1)}
              disabled={isToday}
            >
              &#8594;
            </button>
          </div>
          <div className="flex gap-2">
            <button
              className={`${dateChipBase} ${isYesterday ? dateChipSelected : dateChipOff}`}
              onClick={() => setDate(yesterday())}
            >
              Yesterday
            </button>
            <button
              className={`${dateChipBase} ${isToday ? dateChipSelected : dateChipOff}`}
              onClick={() => setDate(dateOnly(new Date()))}
            >
              Today
            </button>
          </div>
        </div>
        <div className="grid grid-cols-3 gap-2.5">
          <div className="col-span-3 rounded-[14px] bg-neutral-800 px-2.5 py-3.5 text-center font-bold text-white">
            <div className="flex items-center justify-center gap-4">
              <div className="flex items-center text-[56px]">
                {digitBox(0)}
                {digitBox(1)}:{digitBox(2)}
                {digitBox(3)}
              </div>
              {isUnambiguous24Hour ? (
                <span className="rounded-lg bg-white px-2.5 py-2 text-sm font-bold tracking-wider text-neutral-800">
                  24h
                </span>
              ) : (
                <div className="flex flex-col gap-1">
                  <button
                    className={`${ampmMiniBase} ${ampm === "AM" ? ampmMiniSelected : ampmMiniOff}`}
                    onClick={() => setAmpm("AM")}
                  >
                    AM
                  </button>
                  <button
                    className={`${ampmMiniBase} ${ampm === "PM" ? ampmMiniSelected : ampmMiniOff}`}
                    onClick={() => setAmpm("PM")}
                  >
                    PM
                  </button>
                </div>
              )}
            </div>
          </div>
          <button className={keyDigitBtn} onClick={() => button("1")}>
            1
          </button>
          <button className={keyDigitBtn} onClick={() => button("2")}>
            2
          </button>
          <button className={keyDigitBtn} onClick={() => button("3")}>
            3
          </button>
          <button className={keyDigitBtn} onClick={() => button("4")}>
            4
          </button>
          <button className={keyDigitBtn} onClick={() => button("5")}>
            5
          </button>
          <button className={keyDigitBtn} onClick={() => button("6")}>
            6
          </button>
          <button className={keyDigitBtn} onClick={() => button("7")}>
            7
          </button>
          <button className={keyDigitBtn} onClick={() => button("8")}>
            8
          </button>
          <button className={keyDigitBtn} onClick={() => button("9")}>
            9
          </button>
          <button className={keyAuxBtn} onClick={props.onClose}>
            &times;
          </button>
          <button className={keyDigitBtn} onClick={() => button("0")}>
            0
          </button>
          <button className={keyAuxBtn} onClick={() => button("DEL")}>
            DEL
          </button>
          <button
            className={`col-span-3 ${keyConfirmBtn}`}
            disabled={!complete}
            onClick={confirm}
          >
            Confirm
          </button>
        </div>
      </div>
    </Dialog>
  );
}

export default function ScanModalDateTimeV2(props: {
  getShowFunction: (
    show: (
      field: string,
      currentDate: Date,
      currentHours: number,
      currentMinutes: number,
    ) => void,
  ) => void;
  onSave: (field: string, date: Date, value: string) => void;
  onClose?: () => void;
}) {
  const [field, setField] = useState<string | null>(null);
  const [initialDate, setInitialDate] = useState<Date>(() =>
    dateOnly(new Date()),
  );
  const [initialAmPm, setInitialAmPm] = useState<AmPm>("AM");
  const [initialValue, setInitialValue] = useState<string>("");

  function show(
    fieldName: string,
    currentDate: Date,
    currentHours: number,
    currentMinutes: number,
  ) {
    setInitialDate(currentDate);
    setInitialAmPm(currentHours >= 12 ? "PM" : "AM");
    setInitialValue(to12HourDigits(currentHours, currentMinutes));
    setField(fieldName);
  }

  useEffect(() => {
    props.getShowFunction(show);
  });

  function onSave(fieldName: string, date: Date, value: string) {
    props.onSave(fieldName, date, value);
    setField(null);
  }

  function onClose() {
    setField(null);
    if (props.onClose) {
      props.onClose();
    }
  }

  if (field === null) {
    return null;
  }

  return (
    <Inner
      key={field}
      onSave={onSave}
      onClose={onClose}
      field={field}
      initialDate={initialDate}
      initialAmPm={initialAmPm}
      initialValue={initialValue}
    />
  );
}

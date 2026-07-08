import { useEffect, useState } from "react";
import { formatDayDate, isSameDay } from "../../lib/time";

type AmPm = "AM" | "PM";

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
  "shrink-0 rounded-xl bg-neutral-800 px-4 py-2.5 text-2xl text-white shadow-md disabled:cursor-default disabled:opacity-30";
const dateChipBase = "flex-1 rounded-[10px] border-2 p-2.5 text-xl shadow-sm";
const dateChipOff = "border-neutral-300 bg-white text-neutral-700";
const dateChipSelected = "border-accent bg-accent text-white";
const digitSpanBase =
  "mx-1 inline-block box-border w-[52px] rounded-[10px] border-4 border-transparent bg-white text-center text-neutral-800";
const ampmMiniBase = "rounded-lg border-2 px-3 py-[3px] text-base shadow-none";
const ampmMiniOff = "border-neutral-300 bg-white text-neutral-700";
const ampmMiniSelected = "border-accent bg-accent text-white";
const keyDigitBtn =
  "block w-40 cursor-pointer rounded-[14px] bg-neutral-800 px-2.5 py-[18px] text-[64px] text-white no-underline shadow-md active:bg-neutral-600";
const keyAuxBtn =
  "block w-40 cursor-pointer rounded-[14px] bg-neutral-200 px-2.5 py-[18px] text-[32px] text-neutral-700 no-underline shadow-md active:bg-neutral-300";
const keyConfirmBtn =
  "block w-full cursor-pointer rounded-[14px] bg-[#2f7d4f] px-2.5 py-[18px] text-[40px] text-white no-underline shadow-md active:bg-[#276a43] disabled:cursor-default disabled:bg-neutral-300 disabled:text-neutral-500 disabled:shadow-none";

export function Inner(props: {
  show: boolean;
  onSave: (field: string, date: Date, value: string) => void;
  onClose: () => void;
  field: string;
  initialDate: Date;
  initialAmPm: AmPm;
  initialValue: string;
}) {
  const [value, setValue] = useState<string>(props.initialValue);
  const [ampm, setAmpm] = useState<AmPm>(props.initialAmPm);
  const [date, setDate] = useState<Date>(props.initialDate);

  const isToday = isSameDay(date, new Date());
  const isYesterday = isSameDay(date, yesterday());

  // "00" or "13"-"23" can only mean 24-hour time; "01"-"12" is ambiguous and
  // needs the AM/PM toggle to resolve
  const hourEntered =
    value.length >= 2 ? parseInt(value.slice(0, 2), 10) : null;
  const isUnambiguous24Hour =
    hourEntered !== null && (hourEntered === 0 || hourEntered >= 13);

  function button(key: string) {
    if (key === "DEL") {
      setValue(value.slice(0, -1));
      return;
    }
    // field opens prefilled with the current time — typing a digit while it's
    // still full starts a fresh entry rather than being a no-op
    if (value.length >= 4) {
      if (Number(key) <= 2) {
        setValue(key);
      }
      return;
    }
    if (validate(Number(key))) {
      setValue(value + key);
    }
  }

  function validate(key: number): boolean {
    const position = value.length;
    // hour digits: 00-23 ("00" and "13"-"23" mean 24-hour time)
    if (position === 0 && key > 2) return false;
    if (position === 1 && value.charAt(0) === "2" && key > 3) return false;
    // minute digits: 00-59
    if (position === 2 && key > 5) return false;

    return true;
  }

  function char(index: number): string {
    if (index < value.length) {
      return value.charAt(index);
    }
    return "\xa0"; // non-breaking space
  }

  function current(index: number): boolean {
    return index === value.length;
  }

  function changeDay(delta: number) {
    const next = new Date(date);
    next.setDate(next.getDate() + delta);
    if (dateOnly(next) > dateOnly(new Date())) {
      return;
    }
    setDate(dateOnly(next));
  }

  function confirm() {
    const hourTyped = parseInt(value.slice(0, 2), 10);
    const minutes = parseInt(value.slice(2, 4), 10);
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
    <div
      className="fixed inset-0 z-2 items-center justify-center"
      onClick={(e) => {
        if (e.target === e.currentTarget) {
          props.onClose();
        }
      }}
      style={{
        display: props.show ? "flex" : "none",
      }}
    >
      <div className="rounded-[20px] bg-[#f4f4f4] p-5 shadow-[0_20px_50px_rgba(0,0,0,0.35)]">
        <div>
          <div className="mb-3.5 w-full">
            <div className="mb-2 flex items-center gap-2">
              <button className={dateBtnClasses} onClick={() => changeDay(-1)}>
                &#8592;
              </button>
              <span className="flex-1 text-center text-[26px] font-bold text-neutral-800">
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
          <table className="border-separate border-spacing-2.5">
            <tbody>
              <tr>
                <th
                  colSpan={3}
                  className="rounded-[14px] bg-neutral-800 px-2.5 py-3.5 text-white"
                >
                  <div className="flex items-center justify-center gap-4">
                    <div className="flex items-center text-[56px]">
                      <span
                        className={`${digitSpanBase} ${current(0) ? "border-accent" : ""}`}
                      >
                        {char(0)}
                      </span>
                      <span
                        className={`${digitSpanBase} ${current(1) ? "border-accent" : ""}`}
                      >
                        {char(1)}
                      </span>
                      :
                      <span
                        className={`${digitSpanBase} ${current(2) ? "border-accent" : ""}`}
                      >
                        {char(2)}
                      </span>
                      <span
                        className={`${digitSpanBase} ${current(3) ? "border-accent" : ""}`}
                      >
                        {char(3)}
                      </span>
                    </div>
                    {isUnambiguous24Hour ? (
                      <span className="rounded-lg bg-white px-2.5 py-2 text-sm font-bold tracking-[0.05em] text-neutral-800">
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
                </th>
              </tr>
              <tr>
                <td className="p-0">
                  <button className={keyDigitBtn} onClick={() => button("1")}>
                    1
                  </button>
                </td>
                <td className="p-0">
                  <button className={keyDigitBtn} onClick={() => button("2")}>
                    2
                  </button>
                </td>
                <td className="p-0">
                  <button className={keyDigitBtn} onClick={() => button("3")}>
                    3
                  </button>
                </td>
              </tr>
              <tr>
                <td className="p-0">
                  <button className={keyDigitBtn} onClick={() => button("4")}>
                    4
                  </button>
                </td>
                <td className="p-0">
                  <button className={keyDigitBtn} onClick={() => button("5")}>
                    5
                  </button>
                </td>
                <td className="p-0">
                  <button className={keyDigitBtn} onClick={() => button("6")}>
                    6
                  </button>
                </td>
              </tr>
              <tr>
                <td className="p-0">
                  <button className={keyDigitBtn} onClick={() => button("7")}>
                    7
                  </button>
                </td>
                <td className="p-0">
                  <button className={keyDigitBtn} onClick={() => button("8")}>
                    8
                  </button>
                </td>
                <td className="p-0">
                  <button className={keyDigitBtn} onClick={() => button("9")}>
                    9
                  </button>
                </td>
              </tr>
              <tr>
                <td className="p-0">
                  <button className={keyAuxBtn} onClick={props.onClose}>
                    &times;
                  </button>
                </td>
                <td className="p-0">
                  <button className={keyDigitBtn} onClick={() => button("0")}>
                    0
                  </button>
                </td>
                <td className="p-0">
                  <button className={keyAuxBtn} onClick={() => button("DEL")}>
                    DEL
                  </button>
                </td>
              </tr>
              <tr>
                <td colSpan={3} className="p-0">
                  <button
                    className={keyConfirmBtn}
                    disabled={value.length < 4}
                    onClick={confirm}
                  >
                    Confirm
                  </button>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>
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
  const shouldShow = field !== null;

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

  return (
    <>
      <div
        className="fixed inset-0 z-1 bg-black transition-opacity duration-500"
        style={{
          display: shouldShow ? "block" : "none",
          opacity: shouldShow ? 0.5 : 0,
        }}
      ></div>
      {field && (
        <Inner
          key={field}
          show={shouldShow}
          onSave={onSave}
          onClose={onClose}
          field={field}
          initialDate={initialDate}
          initialAmPm={initialAmPm}
          initialValue={initialValue}
        />
      )}
    </>
  );
}

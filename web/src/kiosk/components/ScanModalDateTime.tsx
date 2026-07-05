import { useEffect, useState } from "react";

const keypadDigitBase =
  "m-[5px] inline-block w-[45px] rounded-lg border border-neutral-400 bg-white px-2.5 transition-colors duration-200";
const keypadButtonBase =
  "block w-40 cursor-pointer bg-neutral-100 px-2.5 py-[18px] text-[64px] text-black no-underline hover:bg-neutral-300 active:bg-neutral-400";

export function Inner(props: {
  show: boolean;
  onSave: (field: string, value: string) => void;
  onClose: () => void;
  field: string;
}) {
  const [value, setValue] = useState<string>("");

  function button(key: string) {
    if (key === "DEL") {
      setValue(value.slice(0, -1));
    } else {
      if (validate(Number(key))) {
        const newValue = value + key;
        setValue(newValue);
        if (newValue.length === 4) {
          props.onSave(props.field, newValue);
        }
      }
    }
  }

  function validate(key: number): boolean {
    const position = value.length;
    if (position == 0 && key > 2) return false;
    if (position == 1 && value.charAt(0) == "2" && key > 3) return false;
    if (position == 2 && key > 5) return false;

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

  return (
    <div
      className="fixed inset-0 z-2 items-center justify-center"
      style={{
        display: props.show ? "flex" : "none",
      }}
    >
      <div className="rounded-xl bg-white shadow-2xl">
        <table className="border-collapse">
          <tbody>
            <tr>
              <th
                colSpan={3}
                className="border border-neutral-300 bg-neutral-200 p-2.5 text-[64px]"
              >
                <span
                  className={`${keypadDigitBase} ${current(0) ? "m-px border-[5px] border-accent" : ""}`}
                >
                  {char(0)}
                </span>
                <span
                  className={`${keypadDigitBase} ${current(1) ? "m-px border-[5px] border-accent" : ""}`}
                >
                  {char(1)}
                </span>
                :
                <span
                  className={`${keypadDigitBase} ${current(2) ? "m-px border-[5px] border-accent" : ""}`}
                >
                  {char(2)}
                </span>
                <span
                  className={`${keypadDigitBase} ${current(3) ? "m-px border-[5px] border-accent" : ""}`}
                >
                  {char(3)}
                </span>
              </th>
            </tr>
            <tr>
              <td className="border border-neutral-300 p-0">
                <button
                  className={keypadButtonBase}
                  onClick={() => button("1")}
                >
                  1
                </button>
              </td>
              <td className="border border-neutral-300 p-0">
                <button
                  className={keypadButtonBase}
                  onClick={() => button("2")}
                >
                  2
                </button>
              </td>
              <td className="border border-neutral-300 p-0">
                <button
                  className={keypadButtonBase}
                  onClick={() => button("3")}
                >
                  3
                </button>
              </td>
            </tr>
            <tr>
              <td className="border border-neutral-300 p-0">
                <button
                  className={keypadButtonBase}
                  onClick={() => button("4")}
                >
                  4
                </button>
              </td>
              <td className="border border-neutral-300 p-0">
                <button
                  className={keypadButtonBase}
                  onClick={() => button("5")}
                >
                  5
                </button>
              </td>
              <td className="border border-neutral-300 p-0">
                <button
                  className={keypadButtonBase}
                  onClick={() => button("6")}
                >
                  6
                </button>
              </td>
            </tr>
            <tr>
              <td className="border border-neutral-300 p-0">
                <button
                  className={keypadButtonBase}
                  onClick={() => button("7")}
                >
                  7
                </button>
              </td>
              <td className="border border-neutral-300 p-0">
                <button
                  className={keypadButtonBase}
                  onClick={() => button("8")}
                >
                  8
                </button>
              </td>
              <td className="border border-neutral-300 p-0">
                <button
                  className={keypadButtonBase}
                  onClick={() => button("9")}
                >
                  9
                </button>
              </td>
            </tr>
            <tr>
              <td className="border border-neutral-300 p-0">
                <button className={keypadButtonBase} onClick={props.onClose}>
                  &times;
                </button>
              </td>
              <td className="border border-neutral-300 p-0">
                <button
                  className={keypadButtonBase}
                  onClick={() => button("0")}
                >
                  0
                </button>
              </td>
              <td className="border border-neutral-300 p-0">
                <button
                  className={keypadButtonBase}
                  onClick={() => button("DEL")}
                >
                  DEL
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  );
}

export default function ScanModalDateTime(props: {
  getShowFunction: (show: (field: string) => void) => void;
  onSave: (field: string, value: string) => void;
  onClose?: () => void;
}) {
  const [field, setField] = useState<string | null>(null);
  const shouldShow = field !== null;

  function show(field: string) {
    setField(field);
  }

  useEffect(() => {
    props.getShowFunction(show);
  });

  function onSave(field: string, value: string) {
    props.onSave(field, value);
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
          show={shouldShow}
          onSave={onSave}
          onClose={onClose}
          field={field}
        />
      )}
    </>
  );
}

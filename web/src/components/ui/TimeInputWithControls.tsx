import type { InputHTMLAttributes } from "react";
import TextInput from "./TextInput";
import type { InputWidth } from "./inputStyles";
import { Button } from "./Button";
import { dateToInputDateTimeLocal } from "../../lib/time";

const ADJUSTMENTS = [
  { label: "-1d", ms: -86_400_000 },
  { label: "-1h", ms: -3_600_000 },
  { label: "+1h", ms: 3_600_000 },
  { label: "+1d", ms: 86_400_000 },
] as const;

type Props = Omit<
  InputHTMLAttributes<HTMLInputElement>,
  "type" | "value" | "onChange" | "width"
> & {
  value: string;
  onChange: (value: string) => void;
  width?: InputWidth;
  /** Renders an extra button that copies another field's value into this one
   * (e.g. an end-time field copying the start time). */
  copyFrom?: { label: string; value: string };
};

/**
 * A datetime-local input with buttons to nudge the value by an hour or a day,
 * and optionally to copy another field's value in. Used for the start/end
 * time pairs across the admin activity forms and report date-range pickers.
 */
export default function TimeInputWithControls({
  value,
  onChange,
  copyFrom,
  ...inputProps
}: Props) {
  function adjust(ms: number) {
    const base = value ? new Date(value) : new Date();
    onChange(dateToInputDateTimeLocal(new Date(base.getTime() + ms)));
  }

  return (
    <div className="flex flex-wrap items-center gap-2">
      <TextInput
        type="datetime-local"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        {...inputProps}
      />
      <div className="flex flex-wrap gap-1">
        {ADJUSTMENTS.map(({ label, ms }) => (
          <Button
            key={label}
            type="button"
            variant="secondary"
            size="row"
            onClick={() => adjust(ms)}
          >
            {label}
          </Button>
        ))}
        {copyFrom && (
          <Button
            type="button"
            variant="secondary"
            size="row"
            disabled={!copyFrom.value}
            onClick={() => onChange(copyFrom.value)}
          >
            {copyFrom.label}
          </Button>
        )}
      </div>
    </div>
  );
}

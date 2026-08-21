import { useState } from "react";
import { dateToInputDateTimeLocal } from "../../lib/time";

export default function useActivityTimeRange() {
  const [defaultRange] = useState(() => {
    const endDate = new Date();
    const startDate = new Date(endDate.getTime() - 30 * 24 * 60 * 60 * 1000);
    return {
      startInput: dateToInputDateTimeLocal(startDate),
      endInput: dateToInputDateTimeLocal(endDate),
      startUnix: Math.floor(startDate.getTime() / 1000),
      endUnix: Math.floor(endDate.getTime() / 1000),
    };
  });

  const [startInput, setStartInput] = useState(defaultRange.startInput);
  const [endInput, setEndInput] = useState(defaultRange.endInput);

  const parsedStartMs = Date.parse(startInput);
  const parsedEndMs = Date.parse(endInput);
  const startTime = Number.isNaN(parsedStartMs)
    ? defaultRange.startUnix
    : Math.floor(parsedStartMs / 1000);
  const endTime = Number.isNaN(parsedEndMs)
    ? defaultRange.endUnix
    : Math.floor(parsedEndMs / 1000);
  const hasValidRange = startTime < endTime;
  const queryStartTime = hasValidRange ? startTime : defaultRange.startUnix;
  const queryEndTime = hasValidRange ? endTime : defaultRange.endUnix;

  // What's actually queried/rendered by callers that gate on an "Update
  // results" button, kept separate from the picker-derived values above so
  // every keystroke in the date inputs doesn't fire its own query.
  const [appliedRange, setAppliedRange] = useState({
    startTime: queryStartTime,
    endTime: queryEndTime,
  });
  const isDirty =
    queryStartTime !== appliedRange.startTime ||
    queryEndTime !== appliedRange.endTime;

  function applyRange() {
    if (!hasValidRange) return;
    setAppliedRange({ startTime: queryStartTime, endTime: queryEndTime });
  }

  return {
    startInput,
    endInput,
    setStartInput,
    setEndInput,
    hasValidRange,
    queryStartTime,
    queryEndTime,
    appliedStartTime: appliedRange.startTime,
    appliedEndTime: appliedRange.endTime,
    isDirty,
    applyRange,
  };
}

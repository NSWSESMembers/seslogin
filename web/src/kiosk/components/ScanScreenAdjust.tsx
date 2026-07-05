import { useRef, useState } from "react";
import ScanModalDateTime from "./ScanModalDateTime";
import { formatDayDate, formatTimeOfDay, isSameDay } from "../../lib/time";
import type { TransactionSignedOut } from "../ScanState";
import { categories as categoriesFixture } from "../../lib/categories";
import { scanView, scanViewPosition, type ScreenPosition } from "../../styles";
import { Button } from "../../components/ui/Button";

type TimeOfDay = { hours: number; minutes: number };

function dateOnly(d: Date): Date {
  const result = new Date(d);
  result.setHours(0, 0, 0, 0);
  return result;
}

function combine(date: Date, time: TimeOfDay, end: boolean): Date {
  const result = new Date(date);
  const sec = end ? 59 : 0;
  const ms = end ? 999 : 0; // this will get rounded down
  result.setHours(time.hours, time.minutes, sec, ms);
  return result;
}

function Inner(props: {
  transaction: TransactionSignedOut;
  onSubmit: (startTime: Date, endTime: Date) => void;
  onEditCategory: () => void;
  isSubmitting: boolean;
}) {
  const transaction = props.transaction;
  const [date, setDate] = useState<Date>(() => dateOnly(transaction.startTime));
  const [startTime, setStartTime] = useState<TimeOfDay>({
    hours: transaction.startTime.getHours(),
    minutes: transaction.startTime.getMinutes(),
  });
  // if endTime is not set and startTime is more than 20h ago, set endTime to startTime + 1h
  const [endTime, setEndTime] = useState<TimeOfDay>(() => {
    const end =
      transaction.endTime ||
      (transaction.startTime.getTime() < Date.now() - 20 * 60 * 60 * 1000
        ? new Date(transaction.startTime.getTime() + 60 * 60 * 1000)
        : new Date());
    return { hours: end.getHours(), minutes: end.getMinutes() };
  });
  const showDateTimeModal = useRef<(field: string) => void | null>(null);

  const startTimeStr = formatTimeOfDay(startTime.hours, startTime.minutes);
  const endTimeStr = formatTimeOfDay(endTime.hours, endTime.minutes);
  const startDayStr = formatDayDate(date);
  const isStartToday = isSameDay(date, new Date());

  function changeStartDay(delta: number) {
    const newDate = new Date(date);
    newDate.setDate(newDate.getDate() + delta);

    const today = new Date();
    if (
      newDate.getFullYear() > today.getFullYear() ||
      (newDate.getFullYear() === today.getFullYear() &&
        newDate.getMonth() > today.getMonth()) ||
      (newDate.getFullYear() === today.getFullYear() &&
        newDate.getMonth() === today.getMonth() &&
        newDate.getDate() > today.getDate())
    ) {
      return;
    }

    setDate(newDate);
  }

  function showModalForField(field: string) {
    // this might not be set due to a race relating to the useEffect in ScanModalDateTime
    showDateTimeModal.current!(field);
  }

  function uponModalSave(field: string, value: string) {
    const hours = parseInt(value.slice(0, 2), 10);
    const minutes = parseInt(value.slice(2, 4), 10);
    if (field === "startTime") {
      setStartTime({ hours, minutes });
    } else if (field === "endTime") {
      setEndTime({ hours, minutes });
    }
  }

  function buildStartDate(): Date {
    return combine(date, startTime, false);
  }

  function buildEndDate(): Date {
    const endSameDay = combine(date, endTime, true);
    const start = buildStartDate();
    if (endSameDay > start) {
      return endSameDay;
    }
    const endNextDay = new Date(endSameDay);
    endNextDay.setDate(endNextDay.getDate() + 1);
    return endNextDay;
  }

  let categoryName = "Unknown";
  let subcategoryName = "Unknown";
  let categoryIcon = "unknown";

  for (const category of categoriesFixture) {
    for (const subcategory of category.subcategories || []) {
      if (subcategory.id === props.transaction.categoryId) {
        categoryName = category.name;
        subcategoryName = subcategory.name;
        categoryIcon = subcategory.icon;
        break;
      }
    }
  }

  function onSubmit() {
    props.onSubmit(buildStartDate(), buildEndDate());
  }

  return (
    <>
      <ScanModalDateTime
        getShowFunction={(show) => {
          showDateTimeModal.current = show;
        }}
        onSave={uponModalSave}
      />
      <h1 className="m-0 mb-6 text-[3em]">Adjust</h1>

      <div className="mx-auto flex w-fit min-w-175 flex-col text-[2em]">
        <div className="flex items-center">
          <div className="min-w-48.75 p-2.5 text-right">Day:</div>
          <div className="flex flex-1 items-center justify-between p-2.5">
            <Button
              variant="kiosk"
              size="bare"
              className="px-3.5 py-1.5 text-[1em]"
              onClick={() => changeStartDay(-1)}
            >
              &#8592;
            </Button>
            <span className="flex-1 text-center">{startDayStr}</span>
            <Button
              variant="kiosk"
              size="bare"
              className="px-3.5 py-1.5 text-[1em]"
              onClick={() => changeStartDay(1)}
              disabled={isStartToday}
            >
              &#8594;
            </Button>
          </div>
        </div>
        <div className="flex items-center">
          <div className="min-w-48.75 p-2.5 text-right">Start time:</div>
          <div className="flex-1 p-2.5 font-mono text-[1.5em]">
            {startTimeStr}
          </div>
          <div className="ml-auto p-2.5">
            <Button
              variant="kiosk"
              size="bare"
              className="px-4.5 py-1.5"
              onClick={() => showModalForField("startTime")}
            >
              Edit
            </Button>
          </div>
        </div>
        <div className="flex items-center">
          <div className="min-w-48.75 p-2.5 text-right">End time:</div>
          <div className="flex-1 p-2.5 font-mono text-[1.5em]">
            {endTimeStr}
          </div>
          <div className="ml-auto p-2.5">
            <Button
              variant="kiosk"
              size="bare"
              className="px-4.5 py-1.5"
              onClick={() => showModalForField("endTime")}
            >
              Edit
            </Button>
          </div>
        </div>
        <div className="flex items-center">
          <div className="min-w-48.75 p-2.5 text-right">Category:</div>
          <div className="flex flex-1 items-center justify-center gap-2.5 p-2.5">
            <img src={`/image/categories-cas/${categoryIcon}.png`} />
            <div className="pr-5 text-left text-xl whitespace-nowrap">
              <div>{categoryName}</div>
              <div>{subcategoryName}</div>
            </div>
          </div>
          <div className="ml-auto p-2.5">
            <Button
              variant="kiosk"
              size="bare"
              className="px-4.5 py-1.5"
              onClick={props.onEditCategory}
            >
              Edit
            </Button>
          </div>
        </div>
      </div>

      <Button
        variant="kiosk"
        size="bare"
        className="mt-10 px-6 py-2.5 text-[42px]"
        onClick={onSubmit}
        disabled={props.isSubmitting}
      >
        {props.isSubmitting ? (
          <span className="inline-block size-8 animate-spin rounded-full border-[3px] border-neutral-300 border-t-menu align-middle motion-reduce:animate-none" />
        ) : (
          "Submit"
        )}
      </Button>
    </>
  );
}

// we expose this wrapper just so we can reset inner state on UUID change without
// causing the container <div> to remount and lose CSS transition state
export default function ScanScreenAdjust(props: {
  transaction: TransactionSignedOut | null;
  uuid: string | null;
  screenPosition: ScreenPosition;
  onEditCategory: () => void;
  onSubmit: (startTime: Date, endTime: Date) => void;
  isSubmitting: boolean;
}) {
  return (
    <div
      className={`${scanView} ${scanViewPosition[props.screenPosition]} inset-y-0 flex flex-col items-center justify-center`}
    >
      {props.transaction && (
        <Inner
          key={props.transaction.uuid}
          transaction={props.transaction}
          onEditCategory={props.onEditCategory}
          onSubmit={props.onSubmit}
          isSubmitting={props.isSubmitting}
        />
      )}
    </div>
  );
}

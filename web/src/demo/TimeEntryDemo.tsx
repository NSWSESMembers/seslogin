import { useRef, useState } from "react";
import ScanModalDateTimeV2 from "../kiosk/components/ScanModalDateTimeV2";
import { formatDayDate } from "../lib/time";

// Standalone harness for the kiosk time-entry modal (ScanModalDateTimeV2), so
// the caret ring, tap-to-edit and keyboard entry can be exercised without
// walking a scan through to the adjust screen. Route: /demo/time.

type Saved = { field: string; date: Date; value: string; at: Date };

export default function TimeEntryDemo() {
  const showModal = useRef<
    | ((
        field: string,
        currentDate: Date,
        currentHours: number,
        currentMinutes: number,
      ) => void)
    | null
  >(null);
  const [saves, setSaves] = useState<Saved[]>([]);
  const [closes, setCloses] = useState(0);

  function open(field: string, hours: number, minutes: number) {
    showModal.current?.(field, new Date(), hours, minutes);
  }

  return (
    <div className="mx-auto flex min-h-screen max-w-200 flex-col gap-6 bg-surface p-8 text-ink">
      <ScanModalDateTimeV2
        getShowFunction={(show) => {
          showModal.current = show;
        }}
        onSave={(field, date, value) =>
          setSaves((prev) => [{ field, date, value, at: new Date() }, ...prev])
        }
        onClose={() => setCloses((n) => n + 1)}
      />

      <div>
        <h1 className="m-0 text-3xl font-bold">Time entry demo</h1>
        <p className="text-ink-muted">
          Tap a digit to move the orange ring onto it, then type to replace it.
          Digits 0-9, Backspace, ←/→, A/P, Enter and Escape all work from a
          hardware keyboard.
        </p>
      </div>

      <div className="flex flex-wrap gap-3">
        <button
          className="rounded-xl bg-accent px-4 py-2.5 text-lg text-white shadow-md"
          onClick={() => open("startTime", 9, 30)}
        >
          Open prefilled (09:30)
        </button>
        <button
          className="rounded-xl bg-accent px-4 py-2.5 text-lg text-white shadow-md"
          onClick={() => open("endTime", 22, 5)}
        >
          Open prefilled 24h (22:05)
        </button>
        <button
          className="rounded-xl bg-accent px-4 py-2.5 text-lg text-white shadow-md"
          onClick={() => open("startTime", new Date().getHours(), 0)}
        >
          Open at this hour
        </button>
      </div>

      <div>
        <h2 className="text-xl font-bold">
          Saves ({saves.length}) · closes without saving ({closes})
        </h2>
        {saves.length === 0 ? (
          <p className="text-ink-muted">Nothing saved yet.</p>
        ) : (
          <ul className="m-0 list-none p-0">
            {saves.map((save, i) => (
              <li key={i} className="border-b border-line py-2 font-mono">
                {save.field} → {formatDayDate(save.date)}{" "}
                {save.value.slice(0, 2)}:{save.value.slice(2, 4)}{" "}
                <span className="text-ink-muted">
                  (raw {JSON.stringify(save.value)})
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

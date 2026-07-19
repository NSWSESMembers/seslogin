import { formatTimeDiff } from "../../lib/time";
import ClientVersionLabel from "../../components/ClientVersionLabel";

// Presentational component: takes plain data so it can be driven by a Relay query
// (Status.tsx) or by mock data (StatusDemo.tsx). The display shape is exported so
// callers can build it.
export type StatusPeriod = {
  id: string;
  startTime: number;
  name: string;
};

function getSignInColor(startTime: number): string {
  const elapsedSeconds = Date.now() / 1000 - startTime;
  if (elapsedSeconds <= 60 * 60 * 6)
    return "text-green-700 dark:text-green-400";
  if (elapsedSeconds <= 60 * 60 * 8) return "text-[#ffcc11]";
  if (elapsedSeconds <= 60 * 60 * 10) return "text-[#ff8000]";
  if (elapsedSeconds <= 60 * 60 * 12) return "text-[#ee4000]";
  return "text-[#880000] dark:text-red-500";
}

type Props = {
  periods: StatusPeriod[];
};

export default function StatusCurrentDisplay({ periods }: Props) {
  const sortedPeriods = [...periods].sort((a, b) => a.startTime - b.startTime);

  return (
    <div className="flex h-dvh flex-col overflow-hidden">
      <div className="relative min-h-0 flex-1">
        <ul className="m-0 h-full list-none columns-2 gap-8 overflow-hidden px-4 py-2 [column-fill:auto]">
          {sortedPeriods.map((period) => (
            <li
              key={period.id}
              className="flex break-inside-avoid items-baseline justify-between gap-4 py-[0.2rem] font-title text-2xl"
            >
              <span className="min-w-0 text-left">{period.name}</span>
              <span
                className={`shrink-0 text-right ${getSignInColor(period.startTime)}`}
              >
                {formatTimeDiff(new Date(period.startTime * 1000), new Date())}
              </span>
            </li>
          ))}
        </ul>
        <div className="absolute right-4 bottom-1 text-xs text-neutral-400">
          <ClientVersionLabel noLink />
        </div>
      </div>
      <div className="bg-neutral-900 px-4 py-2 text-center font-title text-[2rem] text-white">
        {periods.length} member{periods.length !== 1 ? "s" : ""} signed in
      </div>
    </div>
  );
}

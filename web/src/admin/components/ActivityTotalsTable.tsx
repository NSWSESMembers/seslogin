import { formatSeconds } from "../../lib/time";

export type ActivityTotalsRow = {
  id: string;
  name: string;
  totalTime: number;
};

type Props = {
  title: string;
  rows: ReadonlyArray<ActivityTotalsRow>;
};

export default function ActivityTotalsTable({ title, rows }: Props) {
  return (
    <div className="flex-1">
      <h2>{title}</h2>
      <div className="border-t border-neutral-400">
        {rows.map((entry) => (
          <div
            className="flex justify-between gap-3 border-b border-neutral-200 p-1.5"
            key={entry.id}
          >
            <div className="min-w-0">{entry.name}</div>
            <div className="whitespace-nowrap">
              {formatSeconds(entry.totalTime)}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

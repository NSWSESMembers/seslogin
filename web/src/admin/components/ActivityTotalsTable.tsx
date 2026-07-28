import { formatSeconds } from "../../lib/time";

export type ActivityTotalsRow = {
  id: string;
  name: string;
  totalTime: number;
  totalTimeVirtual?: number;
  totalTimeNonVirtual?: number;
};

type Props = {
  title: string;
  rows: ReadonlyArray<ActivityTotalsRow>;
  /** Renders separate Virtual/Non-virtual columns instead of a single total. */
  showSplit?: boolean;
};

export default function ActivityTotalsTable({ title, rows, showSplit }: Props) {
  return (
    <div className="flex-1">
      <h2>{title}</h2>
      {showSplit && (
        <div className="flex justify-end gap-3 text-xs text-ink-muted">
          <span>Virtual</span>
          <span>Non-virtual</span>
        </div>
      )}
      <div className="border-t border-line-strong">
        {rows.map((entry) => (
          <div
            className="flex justify-between gap-3 border-b border-line p-1.5"
            key={entry.id}
          >
            <div className="min-w-0">{entry.name}</div>
            {showSplit ? (
              <div className="flex gap-3 whitespace-nowrap">
                <div className="min-w-15 text-right">
                  {formatSeconds(entry.totalTimeVirtual ?? 0)}
                </div>
                <div className="min-w-15 text-right">
                  {formatSeconds(entry.totalTimeNonVirtual ?? 0)}
                </div>
              </div>
            ) : (
              <div className="whitespace-nowrap">
                {formatSeconds(entry.totalTime)}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

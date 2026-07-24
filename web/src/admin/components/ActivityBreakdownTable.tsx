import { Fragment } from "react";
import { formatSeconds } from "../../lib/time";

export type ActivityBreakdownChildRow = {
  id: string;
  name: string;
  totalTime: number;
  isVirtual?: boolean;
};

export type ActivityBreakdownGroupRow = {
  id: string;
  name: string;
  totalTime: number;
  children: ReadonlyArray<ActivityBreakdownChildRow>;
  /** Optional "X virtual · Y non-virtual" line shown under the group's total. */
  splitLine?: string;
};

type Props = {
  title: string;
  rows: ReadonlyArray<ActivityBreakdownGroupRow>;
};

export default function ActivityBreakdownTable({ title, rows }: Props) {
  return (
    <div className="flex-1">
      <h2>{title}</h2>
      <div className="border-t border-line-strong">
        {rows.map((entry) => (
          <Fragment key={entry.id}>
            <div className="flex justify-between gap-3 border-b border-line p-1.5">
              <div className="min-w-0">{entry.name}</div>
              <div className="text-right">
                <div className="whitespace-nowrap">
                  {formatSeconds(entry.totalTime)}
                </div>
                {entry.splitLine ? (
                  <div className="text-xs text-ink-muted">
                    {entry.splitLine}
                  </div>
                ) : null}
              </div>
            </div>
            {entry.children.map((child) => (
              <div
                key={`${entry.id}-${child.id}`}
                className="flex justify-between gap-3 border-b border-line p-1.5 text-ink-muted"
              >
                <div className="min-w-0 pl-6">
                  {child.name}
                  {child.isVirtual && (
                    <span className="ml-1.5 inline-flex items-center rounded-full bg-blue-100 px-1.5 py-0.25 text-[0.7em] font-semibold text-blue-700 dark:bg-blue-500/15 dark:text-blue-300">
                      Virtual
                    </span>
                  )}
                </div>
                <div className="whitespace-nowrap">
                  {formatSeconds(child.totalTime)}
                </div>
              </div>
            ))}
          </Fragment>
        ))}
      </div>
    </div>
  );
}

import { Fragment } from "react";
import { formatSeconds } from "../../lib/time";

export type ActivityBreakdownChildRow = {
  id: string;
  name: string;
  totalTime: number;
};

export type ActivityBreakdownGroupRow = {
  id: string;
  name: string;
  totalTime: number;
  children: ReadonlyArray<ActivityBreakdownChildRow>;
};

type Props = {
  title: string;
  rows: ReadonlyArray<ActivityBreakdownGroupRow>;
};

export default function ActivityBreakdownTable({ title, rows }: Props) {
  return (
    <div className="flex-1">
      <h2>{title}</h2>
      <div className="border-t border-neutral-400">
        {rows.map((entry) => (
          <Fragment key={entry.id}>
            <div className="flex justify-between gap-3 border-b border-neutral-200 p-1.5">
              <div className="min-w-0">{entry.name}</div>
              <div className="whitespace-nowrap">
                {formatSeconds(entry.totalTime)}
              </div>
            </div>
            {entry.children.map((child) => (
              <div
                key={`${entry.id}-${child.id}`}
                className="flex justify-between gap-3 border-b border-neutral-200 p-1.5 text-neutral-500"
              >
                <div className="min-w-0 pl-6">{child.name}</div>
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

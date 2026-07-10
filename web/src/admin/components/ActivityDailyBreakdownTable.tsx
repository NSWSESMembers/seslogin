import { formatDayDate, formatSeconds } from "../../lib/time";

export type ActivityDailyBreakdownMemberRow = {
  id: string;
  name: string;
  totalTime: number;
};

export type ActivityDailyBreakdownCategoryRow = {
  id: string;
  name: string;
  totalTime: number;
  members: ReadonlyArray<ActivityDailyBreakdownMemberRow>;
};

export type ActivityDailyBreakdownDayRow = {
  date: string;
  totalTime: number;
  categories: ReadonlyArray<ActivityDailyBreakdownCategoryRow>;
};

type Props = {
  days: ReadonlyArray<ActivityDailyBreakdownDayRow>;
};

// "date" is a Sydney-local YYYY-MM-DD string from the API; parsing it with
// `new Date(iso)` reads it as UTC midnight and can shift a day under the
// browser's local timezone, so build the Date from the components instead.
function formatBreakdownDate(iso: string): string {
  const [year, month, day] = iso.split("-").map(Number);
  return formatDayDate(new Date(year, month - 1, day));
}

export default function ActivityDailyBreakdownTable({ days }: Props) {
  if (days.length === 0) {
    return <p>No activity in this range.</p>;
  }

  return (
    <div>
      {days.map((day) => (
        <details
          key={day.date}
          className="group/day border-b border-neutral-600"
        >
          <summary className="flex cursor-pointer list-none justify-between gap-3 px-1.5 py-2 font-bold [&::-webkit-details-marker]:hidden">
            <span className="flex min-w-0 items-center gap-1">
              <span
                aria-hidden
                className="inline-block w-[0.8em] transition-transform group-open/day:rotate-90"
              >
                ▸
              </span>
              {formatBreakdownDate(day.date)}
            </span>
            <span className="whitespace-nowrap">
              {formatSeconds(day.totalTime)}
            </span>
          </summary>

          <div className="pl-5">
            {day.categories.map((category) => (
              <details
                key={category.id}
                className="group/category border-t border-neutral-300"
              >
                <summary className="flex cursor-pointer list-none justify-between gap-3 px-1.5 py-2 [&::-webkit-details-marker]:hidden">
                  <span className="flex min-w-0 items-center gap-1">
                    <span
                      aria-hidden
                      className="inline-block w-[0.8em] transition-transform group-open/category:rotate-90"
                    >
                      ▸
                    </span>
                    {category.name}
                  </span>
                  <span className="whitespace-nowrap">
                    {formatSeconds(category.totalTime)}
                  </span>
                </summary>

                <div className="pl-5">
                  {category.members.map((member) => (
                    <div
                      key={member.id}
                      className="flex justify-between gap-3 border-b border-neutral-200 p-1.5 text-neutral-500"
                    >
                      <div className="min-w-0 pl-6">{member.name}</div>
                      <div className="whitespace-nowrap">
                        {formatSeconds(member.totalTime)}
                      </div>
                    </div>
                  ))}
                </div>
              </details>
            ))}
          </div>
        </details>
      ))}
    </div>
  );
}

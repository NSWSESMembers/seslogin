import { useState } from "react";
import { graphql } from "relay-runtime";
import { useLazyLoadQuery } from "react-relay";
import type { AdminHomeQuery } from "./__generated__/AdminHomeQuery.graphql";
import ClearLocationButton from "../components/ClearLocationButton";
import ShowPasskeyPromptButton from "../components/ShowPasskeyPromptButton";
import DevOnly from "../components/DevOnly";
import PeriodsLineChart from "../components/PeriodsLineChart";
import useSelectedLocation from "../components/useSelectedLocation";
import { formatFullDateTime } from "../../lib/time";

interface DayBucket {
  key: string;
  label: string;
  periodCount: number;
  totalSeconds: number;
}

function toDayKey(date: Date): string {
  const y = date.getFullYear();
  const m = (date.getMonth() + 1).toString().padStart(2, "0");
  const d = date.getDate().toString().padStart(2, "0");
  return `${y}-${m}-${d}`;
}

function formatPct(value: number): string {
  return `${Math.round(value)}%`;
}

function formatHours(seconds: number): string {
  return `${(seconds / 3600).toFixed(1)}h`;
}

function formatSecondsCompact(seconds: number): string {
  const whole = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(whole / 3600)
    .toString()
    .padStart(2, "0");
  const minutes = Math.floor((whole % 3600) / 60)
    .toString()
    .padStart(2, "0");
  const secs = (whole % 60).toString().padStart(2, "0");
  return `${hours}:${minutes}:${secs}`;
}

function formatDayLabel(date: Date): string {
  return date.toLocaleDateString(undefined, {
    weekday: "short",
    month: "short",
    day: "numeric",
  });
}

export default function AdminHome() {
  const selectedLocation = useSelectedLocation();
  const [now] = useState(() => Math.floor(Date.now() / 1000));

  const data = useLazyLoadQuery<AdminHomeQuery>(
    graphql`
      query AdminHomeQuery($location: ID!, $now: Int!) {
        location(id: $location) {
          id
          name
          dashboardSummary(asOf: $now) {
            totalMembers
            activeMembers24H
            activeMembers30D
            checkIns24H
            checkIns7D
            totalTime7D
            avgCompletedDuration7D
            totalKiosks
            onlineKiosks
            recentlyActiveKiosks
            lastSuccessfulMemberSync
            dailyPeriods7D {
              dayStart
              periodCount
              totalTime
            }
            topCategories7D {
              categoryId
              categoryName
              periodCount
              totalTime
            }
          }
        }
      }
    `,
    {
      location: selectedLocation.id,
      now,
    },
    { fetchKey: `${selectedLocation.id}-${now}` },
  );

  const location = data.location;
  const summary = location.dashboardSummary;
  const totalMembers = summary.totalMembers;
  const activeMembers30d = summary.activeMembers30D;
  const activeMembers24h = summary.activeMembers24H;
  const inactiveMembers30d = Math.max(0, totalMembers - activeMembers30d);
  const engagementRate30d =
    totalMembers > 0 ? (activeMembers30d / totalMembers) * 100 : 0;

  const totalKiosks = summary.totalKiosks;
  const onlineKiosks = summary.onlineKiosks;
  const recentlyActiveKiosks = summary.recentlyActiveKiosks;
  const checkIns7d = summary.checkIns7D;
  const checkIns24h = summary.checkIns24H;

  const dayBuckets: DayBucket[] = summary.dailyPeriods7D.map((entry) => {
    const day = new Date(entry.dayStart * 1000);
    return {
      key: toDayKey(day),
      label: formatDayLabel(day),
      periodCount: entry.periodCount,
      totalSeconds: entry.totalTime,
    };
  });

  const totalSeconds7d = summary.totalTime7D;
  const averageDailyCheckIns = checkIns7d / 7;
  const peakDay = [...dayBuckets].sort(
    (a, b) => b.periodCount - a.periodCount,
  )[0];

  const avgCompletedDurationSeconds = summary.avgCompletedDuration7D;
  const topCategories = summary.topCategories7D;
  const maxCategoryPeriods = Math.max(
    1,
    ...topCategories.map((entry) => entry.periodCount),
  );

  const lastSyncText = summary.lastSuccessfulMemberSync
    ? formatFullDateTime(new Date(summary.lastSuccessfulMemberSync * 1000))
    : "Never";

  return (
    <div className="grid gap-4.5">
      <p className="m-0">
        Welcome to the admin dashboard for <strong>{location.name}</strong>.
        This overview highlights member activity and engagement trends at a
        glance.
      </p>

      <section className={CARD_CLASS}>
        <div className={SECTION_TITLE_CLASS}>Quick insights</div>
        <ul className="m-0 grid gap-1.5 pl-4.5">
          <li>
            Peak day this week: <strong>{peakDay.label}</strong> with{" "}
            <strong>{peakDay.periodCount}</strong> periods.
          </li>
          <li>
            Average daily periods:{" "}
            <strong>{averageDailyCheckIns.toFixed(1)}</strong>.
          </li>
          <li>
            Last successful member sync: <strong>{lastSyncText}</strong>.
          </li>
        </ul>
      </section>

      <div className="grid grid-cols-[repeat(auto-fit,minmax(210px,1fr))] gap-3">
        <StatCard
          label="Total members"
          value={totalMembers}
          subtle={`${inactiveMembers30d} inactive in 30d`}
        />
        <StatCard
          label="Active members (30d)"
          value={activeMembers30d}
          subtle={`${formatPct(engagementRate30d)} engagement rate`}
        />
        <StatCard
          label="Active members (24h)"
          value={activeMembers24h}
          subtle={`${checkIns24h} periods in 24h`}
        />
        <StatCard
          label="Kiosks online now"
          value={`${onlineKiosks}/${totalKiosks}`}
          subtle={`${recentlyActiveKiosks} active in last 24h`}
        />
        <StatCard
          label="Periods (7d)"
          value={checkIns7d}
          subtle={`${averageDailyCheckIns.toFixed(1)} per day avg`}
        />
        <StatCard
          label="Activity time (7d)"
          value={formatHours(totalSeconds7d)}
          subtle={`Avg completed period ${formatHours(avgCompletedDurationSeconds)}`}
        />
      </div>

      <section className={CARD_CLASS}>
        <div className={SECTION_TITLE_CLASS}>Periods per day (last 7 days)</div>
        <PeriodsLineChart points={dayBuckets} formatHours={formatHours} />
      </section>

      <section className={CARD_CLASS}>
        <div className={SECTION_TITLE_CLASS}>Top categories (last 7 days)</div>
        {topCategories.length === 0 ? (
          <p className="m-0 text-[#666666]">
            No categorised periods in this window.
          </p>
        ) : (
          <div className="grid grid-cols-1 gap-2.5 min-[781px]:grid-cols-[repeat(auto-fit,minmax(210px,1fr))]">
            {topCategories.map((entry, idx) => {
              const barWidth = `${(entry.periodCount / maxCategoryPeriods) * 100}%`;
              return (
                <article
                  key={entry.categoryId ?? "uncategorised"}
                  className="grid gap-2 rounded-[10px] border border-[#e6e6e6] bg-white p-2.5"
                >
                  <div className="grid grid-cols-[auto_1fr] items-center gap-2">
                    <div className="inline-flex h-6 min-w-8.5 items-center justify-center rounded-full bg-[#f3ece8] text-xs font-bold text-[#aa4f1f]">
                      #{idx + 1}
                    </div>
                    <div className="min-w-0 leading-[1.3] font-semibold">
                      {entry.categoryName}
                    </div>
                  </div>
                  <div className="h-2.5 overflow-hidden rounded-full bg-[#f0f0f0]">
                    <div
                      className="h-full min-w-0.5 rounded-full bg-linear-to-r from-[#c95522] to-[#e8945b]"
                      style={{ width: barWidth }}
                    />
                  </div>
                  <div className="flex flex-wrap gap-1.5">
                    <span className="inline-flex items-center rounded-full bg-[#f8eee9] px-2.25 py-0.75 text-xs font-semibold text-[#953f16]">
                      {entry.periodCount} periods
                    </span>
                    <span className="inline-flex items-center rounded-full bg-[#f3f3f3] px-2.25 py-0.75 text-xs font-semibold text-[#545454]">
                      {formatSecondsCompact(entry.totalTime)}
                    </span>
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </section>

      <DevOnly>
        <ClearLocationButton />
        <ShowPasskeyPromptButton />
      </DevOnly>
    </div>
  );
}

// Shared card chrome for the chart/insights sections and the stat cards.
const CARD_CLASS = "rounded-lg border border-[#d4d4d4] bg-[#fcfcfc] p-3.5";
const SECTION_TITLE_CLASS = "mb-3 text-lg font-bold text-navy";

function StatCard({
  label,
  value,
  subtle,
}: {
  label: string;
  value: string | number;
  subtle: string;
}) {
  return (
    <article className={CARD_CLASS}>
      <div className="mb-1.5 text-[13px] text-[#666666]">{label}</div>
      <div className="text-[26px] leading-[1.1] font-bold text-navy min-[781px]:text-3xl">
        {value}
      </div>
      <div className="mt-1.5 text-[13px] text-[#4f4f4f]">{subtle}</div>
    </article>
  );
}

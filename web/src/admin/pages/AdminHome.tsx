import { useState } from "react";
import { graphql } from "relay-runtime";
import { useLazyLoadQuery } from "react-relay";
import type { AdminHomeQuery } from "./__generated__/AdminHomeQuery.graphql";
import ClearLocationButton from "../components/ClearLocationButton";
import DevOnly from "../components/DevOnly";
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

  const maxDayCount = Math.max(
    1,
    ...dayBuckets.map((bucket) => bucket.periodCount),
  );
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
    <div className="dashboard-home">
      <p className="dashboard-home-intro">
        Welcome to the admin dashboard for <strong>{location.name}</strong>.
        This overview highlights member activity and engagement trends at a
        glance.
      </p>

      <div className="dashboard-stats-grid">
        <article className="dashboard-stat-card">
          <div className="dashboard-stat-label">Total members</div>
          <div className="dashboard-stat-value">{totalMembers}</div>
          <div className="dashboard-stat-subtle">
            {inactiveMembers30d} inactive in 30d
          </div>
        </article>

        <article className="dashboard-stat-card">
          <div className="dashboard-stat-label">Active members (30d)</div>
          <div className="dashboard-stat-value">{activeMembers30d}</div>
          <div className="dashboard-stat-subtle">
            {formatPct(engagementRate30d)} engagement rate
          </div>
        </article>

        <article className="dashboard-stat-card">
          <div className="dashboard-stat-label">Active members (24h)</div>
          <div className="dashboard-stat-value">{activeMembers24h}</div>
          <div className="dashboard-stat-subtle">
            {checkIns24h} periods in 24h
          </div>
        </article>

        <article className="dashboard-stat-card">
          <div className="dashboard-stat-label">Kiosks online now</div>
          <div className="dashboard-stat-value">
            {onlineKiosks}/{totalKiosks}
          </div>
          <div className="dashboard-stat-subtle">
            {recentlyActiveKiosks} active in last 24h
          </div>
        </article>

        <article className="dashboard-stat-card">
          <div className="dashboard-stat-label">Periods (7d)</div>
          <div className="dashboard-stat-value">{checkIns7d}</div>
          <div className="dashboard-stat-subtle">
            {averageDailyCheckIns.toFixed(1)} per day avg
          </div>
        </article>

        <article className="dashboard-stat-card">
          <div className="dashboard-stat-label">Activity time (7d)</div>
          <div className="dashboard-stat-value">
            {formatHours(totalSeconds7d)}
          </div>
          <div className="dashboard-stat-subtle">
            Avg completed period {formatHours(avgCompletedDurationSeconds)}
          </div>
        </article>
      </div>

      <section className="dashboard-chart-card">
        <div className="dashboard-section-title">
          Periods per day (last 7 days)
        </div>
        <div className="dashboard-chart-grid">
          {dayBuckets.map((bucket) => {
            const barWidth = `${(bucket.periodCount / maxDayCount) * 100}%`;
            return (
              <div key={bucket.key} className="dashboard-chart-row">
                <div className="dashboard-chart-day">{bucket.label}</div>
                <div className="dashboard-chart-bar-wrap">
                  <div
                    className="dashboard-chart-bar"
                    style={{ width: barWidth }}
                  />
                </div>
                <div className="dashboard-chart-value">
                  {bucket.periodCount}
                </div>
                <div className="dashboard-chart-subvalue">
                  {formatHours(bucket.totalSeconds)}
                </div>
              </div>
            );
          })}
        </div>
      </section>

      <section className="dashboard-chart-card">
        <div className="dashboard-section-title">
          Top categories (last 7 days)
        </div>
        {topCategories.length === 0 ? (
          <p className="dashboard-empty-state">
            No categorised periods in this window.
          </p>
        ) : (
          <div className="dashboard-category-grid">
            {topCategories.map((entry, idx) => {
              const barWidth = `${(entry.periodCount / maxCategoryPeriods) * 100}%`;
              return (
                <article
                  key={entry.categoryId ?? "uncategorised"}
                  className="dashboard-category-card"
                >
                  <div className="dashboard-category-header">
                    <div className="dashboard-category-rank">#{idx + 1}</div>
                    <div className="dashboard-category-name">
                      {entry.categoryName}
                    </div>
                  </div>
                  <div className="dashboard-category-bar-track">
                    <div
                      className="dashboard-category-bar-fill"
                      style={{ width: barWidth }}
                    />
                  </div>
                  <div className="dashboard-category-meta">
                    <span className="dashboard-category-pill">
                      {entry.periodCount} periods
                    </span>
                    <span className="dashboard-category-pill muted">
                      {formatSecondsCompact(entry.totalTime)}
                    </span>
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </section>

      <section className="dashboard-insights-card">
        <div className="dashboard-section-title">Quick insights</div>
        <ul className="dashboard-insights-list">
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

      <DevOnly>
        <ClearLocationButton />
      </DevOnly>
    </div>
  );
}

import { Suspense, useState } from "react";
import { useSettings } from "../../lib/settings";
import ActivityTimeRange from "../components/ActivityTimeRange";
import ActivityCategorySelector from "../components/ActivityCategorySelector";
import ActivityHeatmapDisplay from "../components/ActivityHeatmapDisplay";
import LoadingIndicator from "../../components/LoadingIndicator";
import { Button } from "../../components/ui/Button";
import Select from "../../components/ui/Select";
import { dateToInputDateTimeLocal } from "../../lib/time";
import {
  MAX_DAY_SCALE_RANGE_DAYS,
  MAX_RANGE_DAYS,
  SCALE_MAX_RANGE_DAYS,
  nearestValidScale,
} from "../components/activityHeatmapBuckets";
import type {
  HeatmapScale,
  HeatmapSortBy,
} from "../components/activityHeatmapBuckets";

const DAY_SECONDS = 24 * 60 * 60;

type HeatmapPreset = "30d" | "90d" | "6m" | "1y" | "max";

const PRESET_ORDER: HeatmapPreset[] = ["30d", "90d", "6m", "1y", "max"];

const PRESET_DAYS: Record<HeatmapPreset, number> = {
  "30d": 30,
  "90d": 90,
  "6m": 180,
  "1y": 365,
  // There is deliberately no "all time" preset: the server caps this query
  // at a year (lifting that would mean building an aggregation table), so
  // the widest thing to offer is the cap itself. Derived from
  // MAX_RANGE_DAYS rather than hardcoded so the widest preset can never
  // drift past what the range check — and the API — will accept.
  max: MAX_RANGE_DAYS,
};

const PRESET_LABELS: Record<HeatmapPreset, string> = {
  "30d": "Last 30 days",
  "90d": "Last 90 days",
  "6m": "Last 6 months",
  "1y": "Last year",
  max: `Max (${MAX_RANGE_DAYS} days)`,
};

const rangeSummaryFormatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
});

function formatRangeSummary(startTime: number, endTime: number): string {
  const days = Math.max(1, Math.round((endTime - startTime) / DAY_SECONDS));
  const start = rangeSummaryFormatter.format(new Date(startTime * 1000));
  const end = rangeSummaryFormatter.format(new Date(endTime * 1000));
  return `${start} – ${end} (${days} day${days === 1 ? "" : "s"})`;
}

function parseDateTimeLocal(value: string): number | null {
  const ms = Date.parse(value);
  return Number.isNaN(ms) ? null : Math.floor(ms / 1000);
}

function sameCategoryIds(a: ReadonlyArray<string>, b: ReadonlyArray<string>) {
  if (a.length !== b.length) return false;
  const bSet = new Set(b);
  return a.every((id) => bSet.has(id));
}

export default function ActivityHeatmap() {
  const settings = useSettings();

  // Single source of truth for the *picker* values: preset chips and the
  // custom-range pickers both write directly to this, so there's never a
  // picker value that disagrees with what's shown in the controls.
  const [endTime, setEndTime] = useState(() => Math.floor(Date.now() / 1000));
  const [startTime, setStartTime] = useState(
    () => endTime - PRESET_DAYS["30d"] * DAY_SECONDS,
  );
  const [preset, setPreset] = useState<HeatmapPreset | "custom">("30d");
  const [categoryIds, setCategoryIds] = useState<string[]>([]);

  // What's actually queried/rendered. Kept separate from the picker values
  // above and only synced on "Update results" — otherwise every filter
  // change (date range, category) fires its own query, so changing several
  // filters means paying for several intermediate, wasted queries. Scale and
  // sort don't affect the query (they only re-bucket/re-sort already-fetched
  // data), so they're applied live below instead of gated on submit.
  const [appliedRange, setAppliedRange] = useState<{
    startTime: number;
    endTime: number;
    categoryIds: string[];
  }>(() => ({
    startTime,
    endTime,
    categoryIds: [],
  }));

  const [scale, setScale] = useState<HeatmapScale>("day");
  const [sortBy, setSortBy] = useState<HeatmapSortBy>("name");

  const maxRangeDays = SCALE_MAX_RANGE_DAYS[scale];
  const rangeDays = (endTime - startTime) / DAY_SECONDS;
  const rangeError =
    startTime >= endTime
      ? "Start time must be before end time."
      : rangeDays > maxRangeDays
        ? scale === "day"
          ? `Day scale supports at most ${MAX_DAY_SCALE_RANGE_DAYS} days at a time — narrow the range or switch to week/month scale.`
          : `Range cannot exceed ${MAX_RANGE_DAYS} days (about a year).`
        : null;
  const hasValidRange = rangeError === null;

  const isDirty =
    startTime !== appliedRange.startTime ||
    endTime !== appliedRange.endTime ||
    !sameCategoryIds(categoryIds, appliedRange.categoryIds);

  function applyPreset(p: HeatmapPreset, now: number) {
    const days = PRESET_DAYS[p];
    setEndTime(now);
    setStartTime(now - days * DAY_SECONDS);
    setPreset(p);
    // A wider preset can put the current scale over its cap — only day
    // scale has one below the API's, so in practice this is day -> week on
    // the 6m/1y/max pills. Coarsen just enough to stay valid rather than
    // making the pill render an error the user has to clear by hand; a
    // scale that still fits is left exactly as they set it.
    setScale((current) => nearestValidScale(current, days));
  }

  function applyFilters() {
    if (!hasValidRange) return;
    setAppliedRange({ startTime, endTime, categoryIds });
  }

  // The applied range can become inconsistent with the (live) scale without
  // going through applyFilters — e.g. a wide week-scale range is applied,
  // then scale is switched straight to "day" — so re-check the combination
  // that's actually about to be rendered, not just the draft/apply gating.
  const appliedRangeDays =
    (appliedRange.endTime - appliedRange.startTime) / DAY_SECONDS;
  const canRenderResults = appliedRangeDays <= maxRangeDays;

  return (
    <>
      <div className="mb-2 flex flex-wrap justify-center gap-2">
        {PRESET_ORDER.map((p) => (
          <Button
            key={p}
            variant={preset === p ? "primary" : "secondary"}
            size="row"
            onClick={() => applyPreset(p, Math.floor(Date.now() / 1000))}
          >
            {PRESET_LABELS[p]}
          </Button>
        ))}
      </div>

      <p className="mb-3 text-center text-sm text-ink-muted">
        Showing{" "}
        <strong className="text-ink">
          {formatRangeSummary(appliedRange.startTime, appliedRange.endTime)}
        </strong>
        {isDirty && hasValidRange && (
          <span className="ml-2 italic">
            (unapplied changes — click "Update results")
          </span>
        )}
      </p>

      <details className="mb-4 text-center text-sm">
        <summary className="cursor-pointer text-ink-muted">
          Custom date range
        </summary>
        <div className="mt-2">
          <ActivityTimeRange
            startInput={dateToInputDateTimeLocal(new Date(startTime * 1000))}
            endInput={dateToInputDateTimeLocal(new Date(endTime * 1000))}
            onStartChange={(value) => {
              const parsed = parseDateTimeLocal(value);
              if (parsed !== null) {
                setStartTime(parsed);
                setPreset("custom");
              }
            }}
            onEndChange={(value) => {
              const parsed = parseDateTimeLocal(value);
              if (parsed !== null) {
                setEndTime(parsed);
                setPreset("custom");
              }
            }}
          />
        </div>
      </details>

      {rangeError && <p className="font-bold text-red-600">{rangeError}</p>}

      <div className="mb-4 flex flex-wrap items-center justify-center gap-5 max-md:flex-col">
        <label className="flex items-center justify-center gap-2">
          Scale
          <Select
            width="auto"
            value={scale}
            onChange={(e) => setScale(e.target.value as HeatmapScale)}
          >
            <option value="day">Day</option>
            <option value="week">Week</option>
            <option value="month">Month</option>
          </Select>
        </label>
        <label className="flex items-center justify-center gap-2">
          Sort by
          <Select
            width="auto"
            value={sortBy}
            onChange={(e) => setSortBy(e.target.value as HeatmapSortBy)}
          >
            <option value="name">Name (A–Z)</option>
            <option value="mostActive">Most active</option>
            <option value="leastActive">Least active</option>
            <option value="mostRecent">Most recently active</option>
            <option value="leastRecent">Least recently active (gaps)</option>
          </Select>
        </label>
        <Suspense fallback={null}>
          <ActivityCategorySelector
            value={categoryIds}
            onChange={setCategoryIds}
          />
        </Suspense>
        <Button
          variant="primary"
          size="row"
          disabled={!hasValidRange || !isDirty}
          onClick={applyFilters}
        >
          Update results
        </Button>
      </div>

      {!canRenderResults && (
        <p className="font-bold text-red-600">
          The applied range is too wide for {scale} scale — narrow the range or
          switch scale, then click "Update results".
        </p>
      )}

      {canRenderResults && (
        <Suspense fallback={<LoadingIndicator />}>
          <ActivityHeatmapDisplay
            locationId={settings?.locationId || ""}
            startTime={appliedRange.startTime}
            endTime={appliedRange.endTime}
            scale={scale}
            categoryIds={appliedRange.categoryIds}
            sortBy={sortBy}
          />
        </Suspense>
      )}
    </>
  );
}

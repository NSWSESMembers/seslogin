import { Suspense, useState } from "react";
import { useSettings } from "../../lib/settings";
import ActivityTimeRange from "../components/ActivityTimeRange";
import ActivityCategorySelector from "../components/ActivityCategorySelector";
import ActivityHeatmapDisplay from "../components/ActivityHeatmapDisplay";
import LoadingIndicator from "../../components/LoadingIndicator";
import { Button } from "../../components/ui/Button";
import Select from "../../components/ui/Select";
import { dateToInputDateTimeLocal } from "../../lib/time";
import type {
  HeatmapScale,
  HeatmapSortBy,
} from "../components/activityHeatmapBuckets";

const DAY_SECONDS = 24 * 60 * 60;

type HeatmapPreset = "30d" | "90d" | "6m" | "1y" | "all";

const PRESET_ORDER: HeatmapPreset[] = ["30d", "90d", "6m", "1y", "all"];

const PRESET_DAYS: Record<HeatmapPreset, number> = {
  "30d": 30,
  "90d": 90,
  "6m": 180,
  "1y": 365,
  // Not literally unbounded — a generous fixed lookback. Precisely "since
  // this location's first period" would need a second query round-trip
  // (location.createdAt fetched before we know what startTime to request);
  // 10 years comfortably covers any real usage of this fairly young system
  // without that complexity.
  all: 10 * 365,
};

const PRESET_LABELS: Record<HeatmapPreset, string> = {
  "30d": "Last 30 days",
  "90d": "Last 90 days",
  "6m": "Last 6 months",
  "1y": "Last year",
  all: "All time",
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

export default function ActivityHeatmap() {
  const settings = useSettings();

  // Single source of truth for the visible range: preset chips and the
  // custom-range pickers both write directly to this, so there's never a
  // picker value that disagrees with what the grid actually shows.
  const [endTime, setEndTime] = useState(() => Math.floor(Date.now() / 1000));
  const [startTime, setStartTime] = useState(
    () => endTime - PRESET_DAYS["30d"] * DAY_SECONDS,
  );
  const [preset, setPreset] = useState<HeatmapPreset | "custom">("30d");

  const [scale, setScale] = useState<HeatmapScale>("day");
  const [categoryId, setCategoryId] = useState("");
  const [sortBy, setSortBy] = useState<HeatmapSortBy>("name");

  const hasValidRange = startTime < endTime;

  function applyPreset(p: HeatmapPreset, now: number) {
    setEndTime(now);
    setStartTime(now - PRESET_DAYS[p] * DAY_SECONDS);
    setPreset(p);
  }

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
          {formatRangeSummary(startTime, endTime)}
        </strong>
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

      {!hasValidRange && (
        <p className="font-bold text-red-600">
          Start time must be before end time.
        </p>
      )}

      <div className="mb-4 flex justify-center gap-5 max-md:flex-col max-md:items-center">
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
            value={categoryId}
            onChange={setCategoryId}
          />
        </Suspense>
      </div>

      {hasValidRange && (
        <Suspense fallback={<LoadingIndicator />}>
          <ActivityHeatmapDisplay
            locationId={settings?.locationId || ""}
            startTime={startTime}
            endTime={endTime}
            scale={scale}
            categoryId={categoryId}
            sortBy={sortBy}
          />
        </Suspense>
      )}
    </>
  );
}

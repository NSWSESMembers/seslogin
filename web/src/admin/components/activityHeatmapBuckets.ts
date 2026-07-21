// Pure date-bucketing logic for the activity heatmap grid (ActivityHeatmapGrid).
// Kept separate from the Relay-fetching display component so it can be unit
// tested without touching GraphQL.

export type HeatmapScale = "day" | "week" | "month";

export interface HeatmapCell {
  totalTime: number;
  periodCount: number;
}

export interface HeatmapColumnGroup {
  /** Stable key for this column (a day/week-start/month date string). */
  key: string;
  /** Short label for the column header, e.g. "Jul 14" or "July 2026". */
  label: string;
  /** Fuller label for tooltips, e.g. "Jul 14 – Jul 20" for a week column. */
  rangeLabel: string;
  /** Inclusive Sydney-local start/end dates ("YYYY-MM-DD") this column covers. */
  startKey: string;
  endKey: string;
}

// The API buckets periods by Sydney-local calendar date
// (`unix_to_sydney_date`, "%Y-%m-%d") — match that exactly so column bounds
// line up with the query result's `date` field. `en-CA` formats as
// YYYY-MM-DD.
const sydneyDateFormatter = new Intl.DateTimeFormat("en-CA", {
  timeZone: "Australia/Sydney",
});

export function sydneyDateKey(unixSeconds: number): string {
  return sydneyDateFormatter.format(new Date(unixSeconds * 1000));
}

function parseDateKey(key: string): Date {
  const [y, m, d] = key.split("-").map(Number);
  return new Date(y, m - 1, d);
}

function formatDateKey(date: Date): string {
  const y = date.getFullYear();
  const m = (date.getMonth() + 1).toString().padStart(2, "0");
  const d = date.getDate().toString().padStart(2, "0");
  return `${y}-${m}-${d}`;
}

// The Sydney-local calendar date of a unix timestamp, as a plain local Date
// usable for day/week/month arithmetic (only its Y/M/D components matter).
function sydneyCalendarDate(unixSeconds: number): Date {
  return parseDateKey(sydneyDateFormatter.format(new Date(unixSeconds * 1000)));
}

function mondayOf(date: Date): Date {
  const dow = date.getDay(); // 0 = Sunday .. 6 = Saturday
  const daysSinceMonday = (dow + 6) % 7;
  const monday = new Date(date);
  monday.setDate(date.getDate() - daysSinceMonday);
  return monday;
}

const dayLabelFormatter = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
});
const monthLabelFormatter = new Intl.DateTimeFormat(undefined, {
  month: "long",
  year: "numeric",
});

// Safety caps against a degenerate/huge range (e.g. a bad custom date), not
// a constraint on any real preset. Day scale is capped much tighter since a
// long range there means one narrow column per day, which stops being a
// useful view long before it becomes a performance problem — a "day" of
// enumeration cost. Week/month buckets are enumerated directly (never via a
// per-day expansion), so even a many-year "All time" range stays cheap: ~10
// years is ~522 weeks or 120 months, both far under their cap.
const MAX_DAY_COLUMNS = 1000;
const MAX_BUCKET_COLUMNS = 5000;

/**
 * Builds the column axis for the requested scale directly (never by
 * expanding to individual days and re-grouping), so zooming out to week or
 * month stays cheap and accurate even over a many-year range — an early
 * per-day expansion would otherwise silently truncate a long "All time"
 * range to only its most recent couple of years before grouping ever ran.
 */
export function buildHeatmapColumns(
  startTime: number,
  endTime: number,
  scale: HeatmapScale,
): HeatmapColumnGroup[] {
  if (scale === "day") {
    const columns: HeatmapColumnGroup[] = [];
    const seen = new Set<string>();
    const dayMs = 24 * 60 * 60 * 1000;
    for (
      let t = startTime * 1000;
      t <= endTime * 1000 && columns.length < MAX_DAY_COLUMNS;
      t += dayMs
    ) {
      const date = new Date(t);
      const key = sydneyDateFormatter.format(date);
      if (seen.has(key)) {
        continue;
      }
      seen.add(key);
      const label = dayLabelFormatter.format(parseDateKey(key));
      columns.push({
        key,
        label,
        rangeLabel: label,
        startKey: key,
        endKey: key,
      });
    }
    return columns;
  }

  const endDate = sydneyCalendarDate(endTime);
  const columns: HeatmapColumnGroup[] = [];

  if (scale === "week") {
    let monday = mondayOf(sydneyCalendarDate(startTime));
    while (monday <= endDate && columns.length < MAX_BUCKET_COLUMNS) {
      const sunday = new Date(monday);
      sunday.setDate(monday.getDate() + 6);
      const startKey = formatDateKey(monday);
      const endKey = formatDateKey(sunday);
      columns.push({
        key: startKey,
        label: dayLabelFormatter.format(monday),
        rangeLabel: `${dayLabelFormatter.format(monday)} – ${dayLabelFormatter.format(sunday)}`,
        startKey,
        endKey,
      });
      monday = new Date(
        monday.getFullYear(),
        monday.getMonth(),
        monday.getDate() + 7,
      );
    }
    return columns;
  }

  // month
  const startDate = sydneyCalendarDate(startTime);
  let cursor = new Date(startDate.getFullYear(), startDate.getMonth(), 1);
  while (cursor <= endDate && columns.length < MAX_BUCKET_COLUMNS) {
    const monthEnd = new Date(cursor.getFullYear(), cursor.getMonth() + 1, 0);
    const startKey = formatDateKey(cursor);
    const endKey = formatDateKey(monthEnd);
    const label = monthLabelFormatter.format(cursor);
    columns.push({ key: startKey, label, rangeLabel: label, startKey, endKey });
    cursor = new Date(cursor.getFullYear(), cursor.getMonth() + 1, 1);
  }
  return columns;
}

/**
 * Finds the column whose [startKey, endKey] contains the given Sydney-local
 * date key, via binary search (columns are sorted, contiguous, non-overlapping).
 */
export function findColumnIndex(
  columns: ReadonlyArray<HeatmapColumnGroup>,
  dateKey: string,
): number {
  let lo = 0;
  let hi = columns.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    const column = columns[mid];
    if (dateKey < column.startKey) {
      hi = mid - 1;
    } else if (dateKey > column.endKey) {
      lo = mid + 1;
    } else {
      return mid;
    }
  }
  return -1;
}

/**
 * Aggregates a person's sparse (date -> cell) activity into cells aligned to
 * `columns`, in one pass over their actual activity — cost is proportional
 * to how much data the person has, not to the length of the visible range.
 */
export function buildRowCells(
  byDay: ReadonlyMap<string, HeatmapCell> | undefined,
  columns: ReadonlyArray<HeatmapColumnGroup>,
): HeatmapCell[] {
  const cells: HeatmapCell[] = columns.map(() => ({
    totalTime: 0,
    periodCount: 0,
  }));
  if (!byDay) {
    return cells;
  }
  for (const [dateKey, cell] of byDay) {
    const idx = findColumnIndex(columns, dateKey);
    if (idx >= 0) {
      cells[idx].totalTime += cell.totalTime;
      cells[idx].periodCount += cell.periodCount;
    }
  }
  return cells;
}

// Number of non-empty color steps in the `--color-heat-*` sequential ramp
// (see app.css) — level 0 means "no activity" and is rendered as a neutral
// empty cell, not the ramp's lightest step, so gaps stay visually distinct
// from genuinely-low activity.
export const HEAT_LEVELS = 5;

/**
 * Maps a cell to a 0-5 intensity level: 0 = no activity, 1-5 = quantized
 * `totalTime` relative to the busiest cell currently on screen. A period
 * that's still open (no total_time yet) still counts as level 1 so an
 * in-progress check-in is visible today, not indistinguishable from a gap.
 */
export function heatLevel(cell: HeatmapCell, maxTotalTime: number): number {
  if (cell.periodCount <= 0) {
    return 0;
  }
  if (cell.totalTime <= 0 || maxTotalTime <= 0) {
    return 1;
  }
  return Math.min(
    HEAT_LEVELS,
    Math.max(1, Math.ceil((cell.totalTime / maxTotalTime) * HEAT_LEVELS)),
  );
}

export type HeatmapSortBy =
  "name" | "mostActive" | "leastActive" | "mostRecent" | "leastRecent";

export interface SortableHeatmapRow {
  name: string;
  cells: readonly HeatmapCell[];
}

function totalTimeOf(row: SortableHeatmapRow): number {
  return row.cells.reduce((sum, cell) => sum + cell.totalTime, 0);
}

// Index of the row's most recent active column, or -1 if never active in
// the visible range (used to surface members who've gone quiet).
function lastActiveIndexOf(row: SortableHeatmapRow): number {
  for (let i = row.cells.length - 1; i >= 0; i--) {
    if (row.cells[i].periodCount > 0) {
      return i;
    }
  }
  return -1;
}

/**
 * Orders heatmap rows for display. `leastActive`/`leastRecent` surface the
 * members most likely to represent a gap (quietest, or longest since last
 * seen) at the top, which is the whole point of a scannable heatmap.
 */
export function sortHeatmapRows<T extends SortableHeatmapRow>(
  rows: readonly T[],
  sortBy: HeatmapSortBy,
): T[] {
  const byName = (a: T, b: T) => a.name.localeCompare(b.name);
  const sorted = [...rows];
  switch (sortBy) {
    case "name":
      return sorted.sort(byName);
    case "mostActive":
      return sorted.sort(
        (a, b) => totalTimeOf(b) - totalTimeOf(a) || byName(a, b),
      );
    case "leastActive":
      return sorted.sort(
        (a, b) => totalTimeOf(a) - totalTimeOf(b) || byName(a, b),
      );
    case "mostRecent":
      return sorted.sort(
        (a, b) => lastActiveIndexOf(b) - lastActiveIndexOf(a) || byName(a, b),
      );
    case "leastRecent":
      return sorted.sort(
        (a, b) => lastActiveIndexOf(a) - lastActiveIndexOf(b) || byName(a, b),
      );
  }
}

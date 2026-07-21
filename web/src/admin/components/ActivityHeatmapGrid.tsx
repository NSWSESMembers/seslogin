import { useRef, useState, type SyntheticEvent } from "react";
import { formatSeconds } from "../../lib/time";
import {
  heatLevel,
  type HeatmapCell,
  type HeatmapColumnGroup,
} from "./activityHeatmapBuckets";

export interface ActivityHeatmapRow {
  personId: string;
  name: string;
  cells: HeatmapCell[];
}

interface ActivityHeatmapGridProps {
  columns: ReadonlyArray<HeatmapColumnGroup>;
  rows: ReadonlyArray<ActivityHeatmapRow>;
}

const NAME_COLUMN_WIDTH_PX = 160;
const CELL_PX = 28;
// Column headers are angled so labels like "July 2026" fit above a narrow
// fixed-width cell column without forcing it wider.
const HEADER_HEIGHT_PX = 80;
const MAX_VIEWPORT_VH = 70;

// Level 0 is a neutral "no activity" cell, deliberately not the ramp's
// lightest step, so a real gap in attendance stays visually distinct from a
// genuinely-quiet cell. Levels 1-5 are the validated `--color-heat-*` ramp
// (see app.css) — one hue, light -> dark, checked against both surfaces with
// the dataviz skill's ordinal-ramp validator.
const LEVEL_CLASSES = [
  "bg-surface-sunken",
  "bg-heat-1",
  "bg-heat-2",
  "bg-heat-3",
  "bg-heat-4",
  "bg-heat-5",
];

function cellLabel(
  rowName: string,
  rangeLabel: string,
  cell: HeatmapCell,
): string {
  if (cell.periodCount <= 0) {
    return `${rowName}, ${rangeLabel}: no activity`;
  }
  const visits =
    cell.periodCount === 1 ? "1 check-in" : `${cell.periodCount} check-ins`;
  return `${rowName}, ${rangeLabel}: ${visits}, ${formatSeconds(cell.totalTime)}`;
}

interface TooltipInfo {
  x: number;
  y: number;
  rowName: string;
  column: HeatmapColumnGroup;
  cell: HeatmapCell;
}

export default function ActivityHeatmapGrid({
  columns,
  rows,
}: ActivityHeatmapGridProps) {
  const wrapperRef = useRef<HTMLDivElement>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  const headerRef = useRef<HTMLDivElement>(null);
  const nameColRef = useRef<HTMLDivElement>(null);
  const [tooltip, setTooltip] = useState<TooltipInfo | null>(null);

  const maxTotalTime = Math.max(
    1,
    ...rows.flatMap((row) => row.cells.map((cell) => cell.totalTime)),
  );

  if (rows.length === 0) {
    return <p className="text-ink-muted">No members at this location.</p>;
  }

  // A single scroll listener keeps the frozen header/name-column panes in
  // sync with the body — this is the whole grid's only scroll-driven work,
  // replacing what would otherwise be one `position: sticky` element per row
  // and per column (Chrome has to re-evaluate every sticky element's
  // position on every scroll frame, which gets visibly janky once there are
  // dozens of members and date columns on screen at once).
  function handleBodyScroll() {
    const body = bodyRef.current;
    if (!body) {
      return;
    }
    if (headerRef.current) {
      headerRef.current.scrollLeft = body.scrollLeft;
    }
    if (nameColRef.current) {
      nameColRef.current.scrollTop = body.scrollTop;
    }
  }

  function showTooltip(
    event: SyntheticEvent<HTMLButtonElement>,
    rowName: string,
    column: HeatmapColumnGroup,
    cell: HeatmapCell,
  ) {
    const wrapperRect = wrapperRef.current?.getBoundingClientRect();
    if (!wrapperRect) {
      return;
    }
    const cellRect = event.currentTarget.getBoundingClientRect();
    setTooltip({
      x: cellRect.left - wrapperRect.left + cellRect.width / 2,
      y: cellRect.top - wrapperRect.top,
      rowName,
      column,
      cell,
    });
  }

  // Shrinks to fit a short member list instead of always reserving the full
  // viewport budget, but caps out (and starts scrolling) past that budget.
  const bodyHeight = `min(calc(${MAX_VIEWPORT_VH}vh - ${HEADER_HEIGHT_PX}px), ${rows.length * CELL_PX}px)`;

  return (
    <div className="grid gap-2">
      <div
        ref={wrapperRef}
        className="relative overflow-hidden rounded-md border border-line"
        role="group"
        aria-label="Attendance heatmap"
      >
        <div
          className="border-r border-b border-line bg-surface"
          style={{
            position: "absolute",
            top: 0,
            left: 0,
            width: NAME_COLUMN_WIDTH_PX,
            height: HEADER_HEIGHT_PX,
            zIndex: columns.length + 10,
          }}
        />

        <div
          ref={headerRef}
          className="overflow-hidden"
          style={{
            position: "absolute",
            top: 0,
            left: NAME_COLUMN_WIDTH_PX,
            right: 0,
            height: HEADER_HEIGHT_PX,
          }}
        >
          <div
            style={{
              display: "grid",
              gridTemplateColumns: `repeat(${columns.length}, ${CELL_PX}px)`,
              height: HEADER_HEIGHT_PX,
              width: columns.length * CELL_PX,
            }}
          >
            {columns.map((column, idx) => (
              <div
                key={column.key}
                className="relative border-b border-line bg-surface"
                // Each column's rotated label overflows rightwards, over
                // its later (in DOM order) neighbours' boxes; a later
                // sibling's opaque background would otherwise always paint
                // over an earlier sibling's overflow, so descending
                // z-index (earlier column on top) keeps every label
                // visible instead of getting clipped by the next column.
                style={{ zIndex: columns.length - idx }}
              >
                <div
                  className="absolute bottom-1 left-1/2 origin-bottom-left -rotate-45 text-xs whitespace-nowrap text-ink-muted"
                  title={column.rangeLabel}
                >
                  {column.label}
                </div>
              </div>
            ))}
          </div>
        </div>

        <div
          ref={nameColRef}
          className="overflow-hidden"
          style={{
            position: "absolute",
            top: HEADER_HEIGHT_PX,
            left: 0,
            width: NAME_COLUMN_WIDTH_PX,
            height: bodyHeight,
          }}
        >
          {rows.map((row) => (
            <div
              key={row.personId}
              className="flex min-w-0 items-center truncate border-b border-line-faint bg-surface px-2 text-sm text-ink"
              style={{ height: CELL_PX }}
              title={row.name}
            >
              {row.name}
            </div>
          ))}
        </div>

        <div
          ref={bodyRef}
          className="overflow-auto"
          style={{
            marginTop: HEADER_HEIGHT_PX,
            marginLeft: NAME_COLUMN_WIDTH_PX,
            height: bodyHeight,
          }}
          onScroll={handleBodyScroll}
        >
          <div
            style={{
              display: "grid",
              gridTemplateColumns: `repeat(${columns.length}, ${CELL_PX}px)`,
              gridAutoRows: CELL_PX,
            }}
          >
            {rows.map((row) =>
              row.cells.map((cell, idx) => {
                const column = columns[idx];
                const level = heatLevel(cell, maxTotalTime);
                return (
                  <button
                    key={`${row.personId}-${column.key}`}
                    type="button"
                    aria-label={cellLabel(row.name, column.rangeLabel, cell)}
                    className={`border-r border-b border-line-faint p-0.5 ${LEVEL_CLASSES[level]} bg-clip-content hover:brightness-110 focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-menu focus-visible:brightness-110`}
                    onMouseEnter={(e) => showTooltip(e, row.name, column, cell)}
                    onFocus={(e) => showTooltip(e, row.name, column, cell)}
                    onMouseLeave={() => setTooltip(null)}
                    onBlur={() => setTooltip(null)}
                  />
                );
              }),
            )}
          </div>
        </div>

        {tooltip && (
          <div
            role="tooltip"
            className="pointer-events-none absolute z-40 -translate-x-1/2 -translate-y-[calc(100%+6px)] rounded-md border border-line bg-surface px-2.5 py-1.5 text-xs whitespace-nowrap text-ink shadow-lg"
            style={{ left: tooltip.x, top: tooltip.y }}
          >
            <div className="font-bold">{tooltip.column.rangeLabel}</div>
            <div>
              {tooltip.cell.periodCount > 0
                ? `${tooltip.cell.periodCount === 1 ? "1 check-in" : `${tooltip.cell.periodCount} check-ins`} · ${formatSeconds(tooltip.cell.totalTime)}`
                : "No activity"}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

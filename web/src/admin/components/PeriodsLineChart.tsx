import { useId, useRef, useState } from "react";

export interface PeriodsLineChartPoint {
  key: string;
  label: string;
  periodCount: number;
  totalSeconds: number;
}

interface PeriodsLineChartProps {
  points: ReadonlyArray<PeriodsLineChartPoint>;
  formatHours: (seconds: number) => string;
}

const VIEW_WIDTH = 640;
const VIEW_HEIGHT = 210;
const MARGIN_LEFT = 34;
const MARGIN_RIGHT = 14;
const MARGIN_TOP = 20;
const MARGIN_BOTTOM = 28;
const INNER_WIDTH = VIEW_WIDTH - MARGIN_LEFT - MARGIN_RIGHT;
const INNER_HEIGHT = VIEW_HEIGHT - MARGIN_TOP - MARGIN_BOTTOM;

const LINE_COLOR = "var(--color-menu)";
const GRID_COLOR = "#e8e8e8";
const AXIS_TEXT_COLOR = "#767676";
const CROSSHAIR_COLOR = "#b7b7b7";
const ACTIVE_DOT_COLOR = "#a8451a";

function niceScale(maxValue: number): { max: number; ticks: number[] } {
  if (maxValue <= 0) {
    return { max: 4, ticks: [0, 1, 2, 3, 4] };
  }
  const roughStep = maxValue / 4;
  const magnitude = 10 ** Math.floor(Math.log10(roughStep));
  const residual = roughStep / magnitude;
  const niceStep =
    residual <= 1 ? 1 : residual <= 2 ? 2 : residual <= 5 ? 5 : 10;
  const step = niceStep * magnitude;
  const max = Math.ceil(maxValue / step) * step;
  const ticks: number[] = [];
  for (let t = 0; t <= max + step / 2; t += step) {
    ticks.push(Math.round(t));
  }
  return { max, ticks };
}

export default function PeriodsLineChart({
  points,
  formatHours,
}: PeriodsLineChartProps) {
  const tooltipId = useId();
  const svgRef = useRef<SVGSVGElement>(null);
  const [activeIndex, setActiveIndex] = useState<number | null>(null);

  const maxCount = Math.max(1, ...points.map((p) => p.periodCount));
  const { max: yMax, ticks } = niceScale(maxCount);
  const stepX =
    points.length > 1 ? INNER_WIDTH / (points.length - 1) : INNER_WIDTH;

  const plotted = points.map((point, index) => ({
    ...point,
    x: MARGIN_LEFT + index * stepX,
    y: MARGIN_TOP + INNER_HEIGHT * (1 - point.periodCount / yMax),
  }));

  const linePath = plotted
    .map((p, i) => `${i === 0 ? "M" : "L"} ${p.x.toFixed(1)} ${p.y.toFixed(1)}`)
    .join(" ");
  const baselineY = MARGIN_TOP + INNER_HEIGHT;
  const areaPath =
    plotted.length > 0
      ? `${linePath} L ${plotted[plotted.length - 1].x.toFixed(1)} ${baselineY} L ${plotted[0].x.toFixed(1)} ${baselineY} Z`
      : "";

  const lastPoint = plotted[plotted.length - 1];

  function setActiveFromClientX(clientX: number) {
    const svg = svgRef.current;
    if (!svg || plotted.length === 0) {
      return;
    }
    const rect = svg.getBoundingClientRect();
    const relativeX = ((clientX - rect.left) / rect.width) * VIEW_WIDTH;
    let nearest = 0;
    let nearestDistance = Infinity;
    plotted.forEach((p, i) => {
      const distance = Math.abs(p.x - relativeX);
      if (distance < nearestDistance) {
        nearestDistance = distance;
        nearest = i;
      }
    });
    setActiveIndex(nearest);
  }

  const active = activeIndex != null ? plotted[activeIndex] : null;

  return (
    <div className="relative aspect-3/2 min-h-35 min-[781px]:aspect-7/2">
      <svg
        ref={svgRef}
        viewBox={`0 0 ${VIEW_WIDTH} ${VIEW_HEIGHT}`}
        className="block size-full touch-none"
        role="img"
        aria-label="Periods per day, last 7 days"
        onPointerMove={(event) => setActiveFromClientX(event.clientX)}
        onPointerLeave={() => setActiveIndex(null)}
      >
        {ticks.map((tick) => {
          const y = MARGIN_TOP + INNER_HEIGHT * (1 - tick / yMax);
          return (
            <g key={tick}>
              <line
                x1={MARGIN_LEFT}
                x2={VIEW_WIDTH - MARGIN_RIGHT}
                y1={y}
                y2={y}
                stroke={GRID_COLOR}
                strokeWidth={1}
                vectorEffect="non-scaling-stroke"
              />
              <text
                x={MARGIN_LEFT - 8}
                y={y}
                textAnchor="end"
                dominantBaseline="middle"
                fill={AXIS_TEXT_COLOR}
                fontSize={11}
              >
                {tick}
              </text>
            </g>
          );
        })}

        {plotted.map((p, i) => (
          <text
            key={p.key}
            x={p.x}
            y={VIEW_HEIGHT - 8}
            textAnchor={
              i === 0 ? "start" : i === plotted.length - 1 ? "end" : "middle"
            }
            fill={AXIS_TEXT_COLOR}
            fontSize={11}
          >
            {p.label}
          </text>
        ))}

        {areaPath ? (
          <path d={areaPath} fill={LINE_COLOR} opacity={0.1} />
        ) : null}
        {linePath ? (
          <path
            d={linePath}
            fill="none"
            stroke={LINE_COLOR}
            strokeWidth={2}
            strokeLinejoin="round"
            strokeLinecap="round"
            vectorEffect="non-scaling-stroke"
          />
        ) : null}

        {active ? (
          <line
            x1={active.x}
            x2={active.x}
            y1={MARGIN_TOP}
            y2={baselineY}
            stroke={CROSSHAIR_COLOR}
            strokeWidth={1}
            vectorEffect="non-scaling-stroke"
          />
        ) : null}

        {plotted.map((p, i) => (
          <circle
            key={p.key}
            cx={p.x}
            cy={p.y}
            r={i === activeIndex ? 5 : 3}
            fill={i === activeIndex ? ACTIVE_DOT_COLOR : LINE_COLOR}
            stroke="#fcfcfc"
            strokeWidth={2}
            vectorEffect="non-scaling-stroke"
          />
        ))}

        {lastPoint ? (
          <text
            x={lastPoint.x}
            y={lastPoint.y - 10}
            textAnchor="end"
            fontSize={12}
            fontWeight={700}
            fill="var(--color-navy)"
          >
            {lastPoint.periodCount}
          </text>
        ) : null}

        {plotted.map((p, i) => (
          <circle
            key={`hit-${p.key}`}
            className="cursor-pointer fill-transparent focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-navy"
            cx={p.x}
            cy={p.y}
            r={12}
            tabIndex={0}
            role="button"
            aria-label={`${p.label}: ${p.periodCount} periods, ${formatHours(p.totalSeconds)}`}
            aria-describedby={i === activeIndex ? tooltipId : undefined}
            onFocus={() => setActiveIndex(i)}
            onBlur={() => setActiveIndex(null)}
            onPointerEnter={() => setActiveIndex(i)}
          />
        ))}
      </svg>

      {active ? (
        <div
          id={tooltipId}
          role="tooltip"
          className="pointer-events-none absolute -translate-x-1/2 -translate-y-[calc(100%+12px)] rounded-md border border-[#d4d4d4] bg-white px-2.5 py-2 text-sm whitespace-nowrap shadow-lg"
          style={{
            left: `${(active.x / VIEW_WIDTH) * 100}%`,
            top: `${(active.y / VIEW_HEIGHT) * 100}%`,
          }}
        >
          <div className="mb-0.5 font-bold text-[#2f2f2f]">{active.label}</div>
          <div className="text-[#2f2f2f]">
            <strong>{active.periodCount}</strong> periods
          </div>
          <div className="text-xs text-[#767676]">
            {formatHours(active.totalSeconds)}
          </div>
        </div>
      ) : null}

      <table className="sr-only">
        <caption>Periods per day, last 7 days</caption>
        <thead>
          <tr>
            <th scope="col">Day</th>
            <th scope="col">Periods</th>
            <th scope="col">Activity time</th>
          </tr>
        </thead>
        <tbody>
          {points.map((point) => (
            <tr key={point.key}>
              <td>{point.label}</td>
              <td>{point.periodCount}</td>
              <td>{formatHours(point.totalSeconds)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

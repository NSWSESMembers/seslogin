import { ribbonLines } from "../lib/trophyRibbon";

// The shield's outline colour, as in its SVG.
const TROPHY_COLOUR = "#9a6d12";

/**
 * The trophy's shield (web/public/image/badges/by-id/trophy-<id>.svg) with the
 * unit's name on a ribbon across it: the trophy belongs to the unit. Shown on
 * the admin Badge Report.
 */
export default function TrophyShield({
  trophyId,
  unitName,
}: {
  trophyId: string;
  unitName: string;
}) {
  const { lines, fontSize } = ribbonLines(unitName);
  // A two-line ribbon is deeper, centred on the same line as a one-line one.
  const top = lines.length === 1 ? 92 : 86;
  const bottom = lines.length === 1 ? 110 : 116;
  const middle = (top + bottom) / 2;
  const lineHeight = fontSize * 1.15;
  return (
    <div className="relative size-32.5">
      <img
        src={`/image/badges/by-id/trophy-${trophyId}.svg`}
        alt=""
        width={130}
        height={130}
        className="size-full"
      />
      <svg
        viewBox="-10 0 140 140"
        className="absolute inset-0 size-full"
        aria-hidden
      >
        <path
          d={`M2 ${top} H118 L112 ${middle} L118 ${bottom} H2 L8 ${middle} Z`}
          fill={TROPHY_COLOUR}
        />
        <text
          textAnchor="middle"
          fontFamily="ui-monospace, Menlo, Consolas, monospace"
          fontSize={fontSize}
          fontWeight={600}
          letterSpacing={fontSize * 0.14}
          fill="#ffffff"
        >
          {lines.map((line, i) => (
            <tspan
              key={line}
              x="60"
              y={
                middle +
                fontSize * 0.36 +
                (i - (lines.length - 1) / 2) * lineHeight
              }
            >
              {line}
            </tspan>
          ))}
        </text>
      </svg>
    </div>
  );
}

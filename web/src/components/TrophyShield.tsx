// The shield's outline colour, as in its SVG.
const TROPHY_COLOUR = "#9a6d12";

/**
 * The trophy's shield (web/public/image/badges/by-id/trophy-<id>.svg) with the
 * unit's name on a ribbon across it: the trophy belongs to the unit. Shown on
 * the admin Badge Report and the kiosk's Trophies overlay.
 */
export default function TrophyShield({
  trophyId,
  unitName,
}: {
  trophyId: string;
  unitName: string;
}) {
  const label = unitName.toUpperCase();
  // The ribbon holds about 16 characters at full size; shrink longer names.
  const fontSize = Math.min(9.5, 150 / Math.max(label.length, 1));
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
          d="M2 92 H118 L112 101 L118 110 H2 L8 101 Z"
          fill={TROPHY_COLOUR}
        />
        <text
          x="60"
          y={101 + fontSize * 0.36}
          textAnchor="middle"
          fontFamily="ui-monospace, Menlo, Consolas, monospace"
          fontSize={fontSize}
          fontWeight={600}
          letterSpacing={fontSize * 0.14}
          fill="#ffffff"
        >
          {label}
        </text>
      </svg>
    </div>
  );
}

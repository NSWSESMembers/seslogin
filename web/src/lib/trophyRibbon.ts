// How a unit's name fits on the ribbon across a trophy's shield
// (components/TrophyShield.tsx).

// Ribbon text, in the shield's 120-wide drawing units. The ribbon's notched
// ends leave about 96 units between them.
const RIBBON_TEXT_WIDTH = 96;
const MAX_FONT_SIZE = 9.5;
// Below this a single line is too small to read, so the name takes two.
const MIN_ONE_LINE_FONT_SIZE = 6.5;
const MAX_TWO_LINE_FONT_SIZE = 8;
// A monospace character is 0.6 em wide, plus the 0.14 em letter spacing.
const CHAR_WIDTH_EM = 0.74;

/** The font size that fits `chars` characters on the ribbon, at most `max`. */
function fittingFontSize(chars: number, max: number) {
  return Math.min(
    max,
    RIBBON_TEXT_WIDTH / (CHAR_WIDTH_EM * Math.max(chars, 1)),
  );
}

/**
 * The ribbon's text: the unit's name in capitals, without a closing "Unit",
 * which the ribbon already says. One line when it fits readably, else two,
 * split at the space nearest the middle.
 */
export function ribbonLines(unitName: string): {
  lines: string[];
  fontSize: number;
} {
  const label =
    unitName
      .trim()
      .replace(/\s+unit$/i, "")
      .toUpperCase() || unitName.trim().toUpperCase();
  const oneLine = fittingFontSize(label.length, MAX_FONT_SIZE);
  const spaces = [...label.matchAll(/ /g)].map((m) => m.index);
  if (oneLine >= MIN_ONE_LINE_FONT_SIZE || spaces.length === 0) {
    return { lines: [label], fontSize: oneLine };
  }
  const middle = label.length / 2;
  const split = spaces.reduce((best, i) =>
    Math.abs(i - middle) < Math.abs(best - middle) ? i : best,
  );
  const lines = [label.slice(0, split), label.slice(split + 1)];
  const longest = Math.max(...lines.map((line) => line.length));
  return {
    lines,
    fontSize: fittingFontSize(longest, MAX_TWO_LINE_FONT_SIZE),
  };
}

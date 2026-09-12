/**
 * Matching for `Combobox`. Pure — no React, no DOM — so it can be unit tested
 * directly and reused by anything else that filters a long option list.
 *
 * The lists this exists for are the ~171 enabled categories, whose names are
 * long and share a small prefix vocabulary: "Workshop - Participant -
 * Navigation", "Trainer - Field Core Skills", "Assessor - Flood Operator L2
 * (Boat)". Two consequences drive the design:
 *
 *   - The word a user knows is rarely at position 0. Someone after boat-operator
 *     assessing knows "boat" and "operator", not that it files under "Assessor".
 *     So tokens are matched independently and order doesn't matter: "boat
 *     assess" and "assess boat" both hit.
 *   - Plain substring matching fails multi-token queries outright — "part nav"
 *     appears contiguously in no label, yet it's exactly how you narrow 171
 *     options to one.
 *
 * Fuzzy/subsequence matching was rejected: with this many lexically
 * near-identical labels, a 2-3 character subsequence query returns dozens of
 * hits in an order the user can't predict. Predictability beats recall here.
 */

/**
 * One choice in a `Combobox`. Lives here rather than in `Combobox.tsx` so the
 * matching functions don't have to import from the component.
 */
export interface ComboboxOption {
  value: string;
  label: string;
  /** Secondary muted line, also matched by the filter (e.g. a member number). */
  description?: string;
  disabled?: boolean;
}

/**
 * Separators treated as word boundaries. The punctuation matters as much as the
 * whitespace: it's what makes `boat` hit "(Boat)" and `l2` hit "L2".
 */
const WORD_SEPARATOR = /[\s\-/()–—,.]+/;

/** The complement of {@link WORD_SEPARATOR}, for locating words with offsets. */
const WORD = /[^\s\-/()–—,.]+/g;

/** Match quality, lowest first. See {@link scoreOption}. */
const TIER_WHOLE_PREFIX = 0;
const TIER_WORD_PREFIX = 1;
const TIER_SUBSTRING = 2;
/** Added when the match needed the description as well as the label. */
const DESCRIPTION_PENALTY = 10;

export function tokenize(query: string): string[] {
  return query.trim().toLowerCase().split(WORD_SEPARATOR).filter(Boolean);
}

/**
 * `[start, end)` offsets of each word in `text`. Offsets are into `text` as
 * given, so callers must pass an already-lowercased string if they intend to
 * compare against lowercased tokens.
 */
function words(text: string): [number, number][] {
  return [...text.matchAll(WORD)].map((m) => [m.index, m.index + m[0].length]);
}

function prefixesSomeWord(text: string, token: string): boolean {
  return words(text).some(([start]) => text.startsWith(token, start));
}

/**
 * How well `text` matches, or `null` for no match. Lower is better.
 * `text` and `tokens` must both already be lowercased.
 */
function tierFor(text: string, query: string, tokens: string[]): number | null {
  if (text.startsWith(query)) return TIER_WHOLE_PREFIX;
  if (tokens.every((token) => prefixesSomeWord(text, token))) {
    return TIER_WORD_PREFIX;
  }
  if (tokens.every((token) => text.includes(token))) return TIER_SUBSTRING;
  return null;
}

/**
 * Match quality for one option, or `null` if it doesn't match at all.
 *
 * The label is tried first. Only if that fails is the description folded in —
 * as one combined haystack rather than a separate field, so a query spanning
 * both ("alice 1000" against label "Alice Anderson" + member number "10000001")
 * still matches. Any label-only match therefore outranks any match that needed
 * the description.
 */
export function scoreOption(
  query: string,
  tokens: string[],
  option: ComboboxOption,
): number | null {
  const label = option.label.toLowerCase();
  const labelTier = tierFor(label, query, tokens);
  if (labelTier !== null) return labelTier;

  if (!option.description) return null;
  const combined = `${label} ${option.description.toLowerCase()}`;
  const combinedTier = tierFor(combined, query, tokens);
  return combinedTier === null ? null : combinedTier + DESCRIPTION_PENALTY;
}

/**
 * The options matching `query`, best first. An empty query returns every option
 * unchanged. Ties keep the caller's order, which every call site has already
 * sorted alphabetically (`Array.prototype.sort` and `toSorted` are stable).
 */
export function filterOptions(
  query: string,
  options: readonly ComboboxOption[],
): readonly ComboboxOption[] {
  const normalized = query.trim().toLowerCase();
  const tokens = tokenize(query);
  if (tokens.length === 0) return options;

  return options
    .map((option) => ({
      option,
      score: scoreOption(normalized, tokens, option),
    }))
    .filter(
      (scored): scored is { option: ComboboxOption; score: number } =>
        scored.score !== null,
    )
    .sort((a, b) => a.score - b.score)
    .map((scored) => scored.option);
}

/**
 * `[start, end)` ranges of `text` that `query` matched, merged and sorted, for
 * emphasising the part of a long label that made it a hit. Returns `[]` when
 * nothing matches.
 *
 * Computed independently of {@link filterOptions} so highlighting still works
 * when a call site supplies its own filter.
 *
 * Offsets are into `text` as given. Case folding is by `toLowerCase()` with no
 * diacritic stripping: NFD normalisation would change the string's length and
 * silently misalign every offset here.
 */
export function matchRanges(query: string, text: string): [number, number][] {
  const tokens = tokenize(query);
  if (tokens.length === 0) return [];

  const lower = text.toLowerCase();
  // A locale can lowercase to a different length (e.g. "İ" to "i̇"), which
  // would shift every offset past it. Rare enough to simply not highlight.
  if (lower.length !== text.length) return [];

  const ranges: [number, number][] = [];
  for (const token of tokens) {
    // Prefer word-prefix hits: for a single-character token, highlighting every
    // substring occurrence would bold most of the label.
    const wordHits = words(lower)
      .filter(([start]) => lower.startsWith(token, start))
      .map(([start]): [number, number] => [start, start + token.length]);
    if (wordHits.length > 0) {
      ranges.push(...wordHits);
      continue;
    }
    for (let at = lower.indexOf(token); at !== -1;) {
      ranges.push([at, at + token.length]);
      at = lower.indexOf(token, at + 1);
    }
  }

  return mergeRanges(ranges);
}

/** Sorts by start and collapses overlapping or touching ranges into one. */
function mergeRanges(ranges: [number, number][]): [number, number][] {
  const sorted = ranges.toSorted((a, b) => a[0] - b[0]);
  const merged: [number, number][] = [];
  for (const [start, end] of sorted) {
    const last = merged.at(-1);
    if (last && start <= last[1]) {
      last[1] = Math.max(last[1], end);
    } else {
      merged.push([start, end]);
    }
  }
  return merged;
}

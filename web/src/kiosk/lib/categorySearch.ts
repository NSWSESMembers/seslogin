import { categories as allCategories } from "../../lib/categories";
import type { Category } from "../../lib/categories";

/**
 * Searching the kiosk's category list by typing, for the sign-out screens.
 *
 * Matching is on the *start of words* across "Role Activity" (the parent
 * category's name plus the sub-category's), every typed word must match, and case
 * and punctuation don't matter: "stor" finds "Storm & Water: Ground", and
 * "trainer storm" finds the Trainer ones.
 */

/** A search needs at least this many letters or digits: one letter would match
 * almost every category, so the plain tiles stay until there is more to go on. */
export const MIN_SEARCH_LENGTH = 2;

// The roles a member is most likely to mean come first. On a course most people
// are participants and only one or two are instructors, so the instructor roles
// go last: someone unsure should not be steered into Trainer or Assessor. These
// are the ids in lib/categories.ts, in the order to show them; any other parent
// sits in between, in the order the list has them.
const PARTICIPANT_ROLE_IDS = [
  "C6" /* Training */,
  "C10" /* Workshop - Participant */,
];
const INSTRUCTOR_ROLE_IDS = [
  "C9" /* Workshop - Trainer */,
  "C7" /* Trainer */,
  "C8" /* Assessor */,
];

// Acronyms and their long forms, which members use for the same activities. An
// activity whose name starts with the words on the left (compared as whole words)
// can also be found by the words on the right, so "dov" finds Drive Operational
// Vehicles and "vertical rescue" finds VR.
const ALSO_KNOWN_AS: [name: string[], also: string[]][] = [
  [["drive", "operational", "vehicles"], ["dov"]],
  [
    ["storm", "water"],
    ["swdo", "damage", "operations"],
  ],
  [
    ["operate", "comms", "equip"],
    ["oce", "communications", "equipment"],
  ],
  [["field", "core", "skills"], ["fcs"]],
  [["vr"], ["vertical", "rescue"]],
  [["rcr"], ["road", "crash", "rescue"]],
  [["road", "crash", "rescue"], ["rcr"]],
];

/** The words an activity can be found by: its own, plus any acronym or long form. */
function searchableWords(leafWords: string[]): string[] {
  const also = ALSO_KNOWN_AS.filter(([name]) =>
    name.every((word, index) => leafWords[index] === word),
  ).flatMap(([, extra]) => extra);
  return also.length > 0 ? [...leafWords, ...also] : leafWords;
}

export type CategorySearchResult = {
  /** The sub-category's id, which is what gets chosen. */
  id: string;
  name: string;
  icon: string;
  parentId: string;
  parentName: string;
};

export type TextPart = { text: string; match: boolean };

type WordRun = { start: number; end: number; folded: string };

/** A character as lower-case letters and digits, or "" if it has none: accents
 * are dropped, so "é" counts as "e", and punctuation counts as nothing. */
function foldChar(char: string): string {
  return char
    .normalize("NFD")
    .replace(/\p{M}/gu, "")
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]/gu, "");
}

/** The words in `text`, with where each sits in the original and its plain form.
 * A word is a run of letters or digits; everything else separates words. */
function wordRuns(text: string): WordRun[] {
  const runs: WordRun[] = [];
  let current: WordRun | null = null;
  let index = 0;
  for (const char of text) {
    const folded = foldChar(char);
    if (folded !== "") {
      if (current === null) {
        current = { start: index, end: index + char.length, folded };
        runs.push(current);
      } else {
        current.end = index + char.length;
        current.folded += folded;
      }
    } else if (char.normalize("NFD").replace(/\p{M}/gu, "") !== "") {
      // A real separator (space, &, :, -). A lone accent mark is not one.
      current = null;
    }
    index += char.length;
  }
  return runs;
}

function words(text: string): string[] {
  return wordRuns(text).map((run) => run.folded);
}

/** What was typed, as the words to look for. A typed "and" stands for the "&" in
 * names like "Storm & Water", which isn't a word, so it is left out (unless it is
 * all that was typed). */
export function searchTokens(query: string): string[] {
  const typed = words(query);
  const withoutAnd = typed.filter((word) => word !== "and");
  return withoutAnd.length > 0 ? withoutAnd : typed;
}

/** True once there is enough typed to search on (see MIN_SEARCH_LENGTH). */
export function isSearchableQuery(query: string): boolean {
  return [...searchTokens(query).join("")].length >= MIN_SEARCH_LENGTH;
}

/** Where a role sits: its group (0 participant, 1 the rest, 2 instructor) and its
 * place in that group. Lower sorts first. */
function rolePlace(parentId: string, source: Category[]): [number, number] {
  const participant = PARTICIPANT_ROLE_IDS.indexOf(parentId);
  if (participant !== -1) return [0, participant];
  const instructor = INSTRUCTOR_ROLE_IDS.indexOf(parentId);
  if (instructor !== -1) return [2, instructor];
  return [1, source.findIndex((category) => category.id === parentId)];
}

type Scored = {
  result: CategorySearchResult;
  leafHits: number;
  startsWithFirst: boolean;
  rank: number;
  group: number;
};

/**
 * The sub-categories that match `query`, in the order to show them.
 *
 * The role groups are a hard order (participant roles, the rest, instructor roles)
 * and win over how well something matches. Inside a group the better match comes
 * first: more of the typed words found in the activity's own name rather than only
 * in the role's, then an activity that starts with the first word; then A to Z by
 * activity, so the same activity under different roles sits together.
 *
 * Pass `parentId` to look inside one role only.
 */
export function searchCategories(
  query: string,
  options: { parentId?: string | null; source?: Category[] } = {},
): CategorySearchResult[] {
  if (!isSearchableQuery(query)) return [];
  const source = options.source ?? allCategories;
  const tokens = searchTokens(query);
  const scored: Scored[] = [];

  for (const parent of source) {
    if (options.parentId != null && parent.id !== options.parentId) continue;
    const parentWords = words(parent.name);
    const [group, rank] = rolePlace(parent.id, source);
    for (const leaf of parent.subcategories) {
      const leafWords = words(leaf.name);
      const findableWords = searchableWords(leafWords);
      let leafHits = 0;
      let matchesAll = true;
      for (const token of tokens) {
        const inLeaf = findableWords.some((word) => word.startsWith(token));
        const inParent = parentWords.some((word) => word.startsWith(token));
        if (inLeaf) leafHits += 1;
        if (!inLeaf && !inParent) {
          matchesAll = false;
          break;
        }
      }
      if (!matchesAll) continue;
      scored.push({
        result: {
          id: leaf.id,
          name: leaf.name,
          icon: leaf.icon,
          parentId: parent.id,
          parentName: parent.name,
        },
        leafHits,
        startsWithFirst: leafWords[0]?.startsWith(tokens[0]) ?? false,
        group,
        rank,
      });
    }
  }

  scored.sort(
    (a, b) =>
      a.group - b.group ||
      b.leafHits - a.leafHits ||
      Number(b.startsWithFirst) - Number(a.startsWithFirst) ||
      a.result.name.localeCompare(b.result.name) ||
      a.rank - b.rank,
  );
  return scored.map((entry) => entry.result);
}

/**
 * `text` split into plain and matched pieces, for showing the letters that were
 * typed in bold: with "stor", "Storm & Water" becomes "Stor" (matched) and
 * "m & Water" (plain).
 */
export function highlightMatches(text: string, tokens: string[]): TextPart[] {
  const parts: TextPart[] = [];
  let cursor = 0;
  for (const run of wordRuns(text)) {
    const token = tokens
      .filter((candidate) => run.folded.startsWith(candidate))
      .sort((a, b) => b.length - a.length)[0];
    if (token === undefined) continue;
    // Walk the original letters until the typed length is covered, so an accented
    // letter that folds to one character is counted once.
    let covered = 0;
    let end = run.start;
    for (const char of text.slice(run.start, run.end)) {
      if (covered >= token.length) break;
      covered += foldChar(char).length;
      end += char.length;
    }
    if (run.start > cursor) {
      parts.push({ text: text.slice(cursor, run.start), match: false });
    }
    parts.push({ text: text.slice(run.start, end), match: true });
    cursor = end;
  }
  if (cursor < text.length) {
    parts.push({ text: text.slice(cursor), match: false });
  }
  return parts;
}

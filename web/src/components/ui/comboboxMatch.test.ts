import { describe, expect, test } from "vitest";
import { filterOptions, matchRanges } from "./comboboxMatch";
import type { ComboboxOption } from "./comboboxMatch";

/**
 * Fixtures are real production category names (and their shape), because the
 * whole design of the matcher is a response to them: long, prefix-heavy, and
 * sharing a small vocabulary.
 */
const BOAT = "Assessor - Flood Operator L2 (Boat)";
const TRAINER = "Trainer - Field Core Skills";
const NAVIGATION = "Workshop - Participant - Navigation";
const STORM = "Workshop - Participant - Storm Damage";
const NESTED_TRAINER = "Workshop - Trainer - Navigation";
const RETRAINER = "Retrainer Refresh";

function opts(...labels: string[]): ComboboxOption[] {
  return labels.map((label) => ({ value: label, label }));
}

const CATEGORIES = opts(BOAT, TRAINER, NAVIGATION, STORM);

function labels(matched: readonly ComboboxOption[]): string[] {
  return matched.map((o) => o.label);
}

describe("filterOptions", () => {
  test("an empty query returns every option in the caller's order", () => {
    expect(filterOptions("", CATEGORIES)).toBe(CATEGORIES);
    expect(labels(filterOptions("   ", CATEGORIES))).toEqual([
      BOAT,
      TRAINER,
      NAVIGATION,
      STORM,
    ]);
  });

  test("matches a whole-label prefix", () => {
    expect(labels(filterOptions("work", CATEGORIES))).toEqual([
      NAVIGATION,
      STORM,
    ]);
  });

  test("matches a word anywhere in the label, not just at the start", () => {
    expect(labels(filterOptions("nav", CATEGORIES))).toEqual([NAVIGATION]);
  });

  test("multi-token queries match regardless of token order", () => {
    // Neither of these occurs contiguously anywhere, which is why plain
    // substring matching is not enough.
    expect(labels(filterOptions("part nav", CATEGORIES))).toEqual([NAVIGATION]);
    expect(labels(filterOptions("nav part", CATEGORIES))).toEqual([NAVIGATION]);
  });

  test("punctuation is a word boundary, so a parenthesised word matches", () => {
    expect(labels(filterOptions("boat", CATEGORIES))).toEqual([BOAT]);
    expect(labels(filterOptions("l2 boat", CATEGORIES))).toEqual([BOAT]);
  });

  test("falls back to a substring match inside a word", () => {
    expect(labels(filterOptions("erator", CATEGORIES))).toEqual([BOAT]);
  });

  test("a query matching nothing returns nothing", () => {
    expect(filterOptions("xyz", CATEGORIES)).toEqual([]);
    expect(filterOptions("nav storm", CATEGORIES)).toEqual([]);
  });

  test("is case insensitive in both directions", () => {
    expect(labels(filterOptions("WORKSHOP", CATEGORIES))).toEqual([
      NAVIGATION,
      STORM,
    ]);
    expect(labels(filterOptions("l2", CATEGORIES))).toEqual([BOAT]);
    expect(labels(filterOptions("L2", CATEGORIES))).toEqual([BOAT]);
  });

  test("ranks a label prefix above a word prefix above a substring", () => {
    const ranked = filterOptions(
      "trainer",
      opts(RETRAINER, NESTED_TRAINER, TRAINER),
    );
    expect(labels(ranked)).toEqual([
      TRAINER, // label starts with the query
      NESTED_TRAINER, // query prefixes a later word
      RETRAINER, // query only appears mid-word
    ]);
  });

  test("ties keep the caller's order", () => {
    expect(labels(filterOptions("workshop", opts(STORM, NAVIGATION)))).toEqual([
      STORM,
      NAVIGATION,
    ]);
  });

  describe("descriptions", () => {
    const MEMBERS: ComboboxOption[] = [
      { value: "p1", label: "Alice Anderson", description: "10000001" },
      { value: "p2", label: "Bob Brown", description: "10000002" },
    ];

    test("are searched, so a member is findable by number", () => {
      expect(labels(filterOptions("10000001", MEMBERS))).toEqual([
        "Alice Anderson",
      ]);
    });

    test("combine with the label, so a query can span both", () => {
      expect(labels(filterOptions("alice 1000", MEMBERS))).toEqual([
        "Alice Anderson",
      ]);
    });

    test("rank below any label match", () => {
      const ranked = filterOptions("1000", [
        ...MEMBERS,
        { value: "c", label: "1000 Hour Award" },
      ]);
      expect(labels(ranked)).toEqual([
        "1000 Hour Award",
        "Alice Anderson",
        "Bob Brown",
      ]);
    });
  });
});

describe("matchRanges", () => {
  /** The slices of `text` that would be highlighted, for readable assertions. */
  function highlighted(query: string, text: string): string[] {
    return matchRanges(query, text).map(([start, end]) =>
      text.slice(start, end),
    );
  }

  test("returns nothing for an empty query or a non-match", () => {
    expect(matchRanges("", NAVIGATION)).toEqual([]);
    expect(matchRanges("   ", NAVIGATION)).toEqual([]);
    expect(matchRanges("xyz", NAVIGATION)).toEqual([]);
  });

  test("marks the matched run of each token, preserving the source case", () => {
    expect(highlighted("nav", NAVIGATION)).toEqual(["Nav"]);
    expect(highlighted("part nav", NAVIGATION)).toEqual(["Part", "Nav"]);
  });

  test("ranges are ordered by position, not by token order", () => {
    expect(highlighted("nav part", NAVIGATION)).toEqual(["Part", "Nav"]);
  });

  test("merges overlapping ranges", () => {
    expect(highlighted("nav navi", "Navigation")).toEqual(["Navi"]);
  });

  test("prefers word starts, so a one-character query stays readable", () => {
    // Not every "a" in the string — only the two that begin a word.
    expect(matchRanges("a", "Alice Anderson")).toEqual([
      [0, 1],
      [6, 7],
    ]);
  });

  test("falls back to every occurrence when no word starts with the token", () => {
    expect(highlighted("erator", BOAT)).toEqual(["erator"]);
  });
});

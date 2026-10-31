import { describe, expect, test } from "vitest";
import { competitionRanks } from "./competitionRank";

const byValue = (n: number) => n;

describe("competitionRanks", () => {
  test("numbers distinct scores sequentially", () => {
    expect(competitionRanks([5, 3, 1], byValue)).toEqual([1, 2, 3]);
  });

  test("ties share a place and the next place skips past them", () => {
    expect(competitionRanks([5, 5, 3, 2, 2, 2, 1], byValue)).toEqual([
      1, 1, 3, 4, 4, 4, 7,
    ]);
  });

  test("everyone tied is first", () => {
    expect(competitionRanks([4, 4, 4], byValue)).toEqual([1, 1, 1]);
  });

  test("an empty list has no ranks", () => {
    expect(competitionRanks([], byValue)).toEqual([]);
  });
});

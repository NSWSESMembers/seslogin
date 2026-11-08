/**
 * Standard competition ranking ("1, 1, 3") for a list already sorted best
 * first: an item tied with the one before it on `score` shares its place, and
 * the next different score takes its row's own 1-based position. `score` may
 * be a string joining several keys, for a tie that needs all of them level.
 * Matches the API's `badgeLeaderboard.rank`, so every badge board numbers ties
 * the same way.
 */
export function competitionRanks<T>(
  sorted: readonly T[],
  score: (item: T) => number | string,
): number[] {
  const ranks: number[] = [];
  sorted.forEach((item, idx) => {
    const tied = idx > 0 && score(sorted[idx - 1]) === score(item);
    ranks.push(tied ? ranks[idx - 1] : idx + 1);
  });
  return ranks;
}

/**
 * Completion-poll cadence for a kiosk waiting to be enrolled, as a function of how long
 * it has been waiting: quick while an admin is likely mid-enrollment, then progressively
 * slower to avoid pointless traffic from a kiosk left on the enrollment screen.
 */
export function pollDelayMs(elapsedMs: number): number {
  if (elapsedMs < 2 * 60 * 1000) return 5 * 1000; // first 2 min: every 5s
  if (elapsedMs < 60 * 60 * 1000) return 60 * 1000; // up to 1h: every minute
  return 5 * 60 * 1000; // after 1h: every 5 min
}

/**
 * How an elapsed sign-in is coloured: green for a normal turnout, warming
 * through amber and orange, red once it has run past the 12 hours at which the
 * scan kiosk starts asking whether the person forgot to sign out (see
 * `FORGOT_SIGN_OUT_PROMPT_THRESHOLD_MS` in ../ScanState).
 *
 * Shared so the full-screen status kiosk and the scan kiosk's signed-in list
 * grade the same duration the same way — two screens that can be side by side
 * in the same room disagreeing about what counts as a long sign-in would read
 * as a bug.
 */
export function signInColorClass(
  startTime: number,
  now: number = Date.now(),
): string {
  const elapsedSeconds = now / 1000 - startTime;
  if (elapsedSeconds <= 60 * 60 * 6)
    return "text-green-700 dark:text-green-400";
  if (elapsedSeconds <= 60 * 60 * 8) return "text-[#ffcc11]";
  if (elapsedSeconds <= 60 * 60 * 10) return "text-[#ff8000]";
  if (elapsedSeconds <= 60 * 60 * 12) return "text-[#ee4000]";
  return "text-[#880000] dark:text-red-500";
}

/**
 * Wording for the member-facing period edit page.
 *
 * A link is sent in two very different situations, and the page has to read
 * correctly for both:
 *
 *   * **complete** — an admin asking the member to check an entry that already
 *     has both times. There is a legitimate "looks right, do nothing" outcome.
 *   * **incomplete** — the member forgot to sign out, so the entry has no finish
 *     time. There is no do-nothing outcome here: until a finish time is entered
 *     the activity doesn't count, so the copy asks for one specific thing.
 *
 * Both sets live here rather than inline in the components so the two readings
 * can be compared side by side, and so the confirmation screen can't drift out
 * of step with the form.
 */
export type PeriodEditCopy = {
  title: string;
  intro: (name: string | null, locationName: string) => string;
  endFieldLabel: string;
  submitLabel: string;
  confirmationTitle: string;
  confirmationIntro: string;
};

const COMPLETE: PeriodEditCopy = {
  title: "Check your time entry",
  intro: (name, locationName) =>
    `${name ? `${name}, if ` : "If "}the times or activity below aren't right, correct them and save. Recorded at ${locationName}.`,
  endFieldLabel: "End time",
  submitLabel: "Save",
  confirmationTitle: "Thank you",
  confirmationIntro:
    "Your time entry has been updated. There's nothing else you need to do.",
};

const INCOMPLETE: PeriodEditCopy = {
  title: "You didn't sign out",
  intro: (name, locationName) =>
    `${name ? `${name}, this ` : "This "}activity at ${locationName} is still open because no finish time was recorded. Enter the time you finished and save — until then it won't count towards your hours.`,
  endFieldLabel: "What time did you finish?",
  submitLabel: "Save finish time",
  confirmationTitle: "Thank you",
  confirmationIntro:
    "Your finish time has been recorded. There's nothing else you need to do.",
};

/**
 * Pick the copy set. `wasIncomplete` must be captured when the page loads, not
 * derived from the saved entry — after a successful save the period always has
 * an end time, so the confirmation would otherwise always show the complete
 * wording no matter how the member arrived.
 */
export function periodEditCopy(wasIncomplete: boolean): PeriodEditCopy {
  return wasIncomplete ? INCOMPLETE : COMPLETE;
}

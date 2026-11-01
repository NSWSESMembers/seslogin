/**
 * The small line under a member's name on the Activity pages, when their primary unit is
 * not the unit being viewed. A member of the viewed unit (a secondary unit of theirs)
 * is not a visitor, so they get their primary unit; anyone else is visiting from theirs.
 */
export function memberUnitLabel(
  isMemberOfViewedUnit: boolean,
  primaryId: string,
  primaryName: string,
  viewedLocationId: string | null | undefined,
): string | null {
  if (primaryId === viewedLocationId) return null;
  return isMemberOfViewedUnit
    ? `Primary: ${primaryName}`
    : `Visiting from ${primaryName}`;
}

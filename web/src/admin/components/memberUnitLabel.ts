/**
 * The small line under a member's name on the Activity pages, when their home unit is
 * not the unit being viewed. A member of the viewed unit (a secondary unit of theirs)
 * is not a visitor, so they get their home unit; anyone else is visiting from theirs.
 */
export function memberUnitLabel(
  isMemberOfViewedUnit: boolean,
  homeId: string,
  homeName: string,
  viewedLocationId: string | null | undefined,
): string | null {
  if (homeId === viewedLocationId) return null;
  return isMemberOfViewedUnit
    ? `Home: ${homeName}`
    : `Visiting from ${homeName}`;
}

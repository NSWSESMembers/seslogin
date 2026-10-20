// An edit form reached from a list should return to that list when it's done,
// not to a fixed page: the Edit button on Incomplete Periods, Previous Periods
// and a member's activity all open the same form. The link passes the list's
// path in router state, and the form navigates back to it after saving.

type ReturnState = { returnTo: string };

export function editReturnState(returnTo: string): ReturnState {
  return { returnTo };
}

// Router state survives a reload and can be set by anything that navigates,
// so only accept a path within the admin site, never another origin.
export function editReturnTo(state: unknown, fallback: string): string {
  const returnTo =
    typeof state === "object" && state !== null && "returnTo" in state
      ? (state as ReturnState).returnTo
      : undefined;
  return typeof returnTo === "string" &&
    returnTo.startsWith("/admin/") &&
    !returnTo.startsWith("//")
    ? returnTo
    : fallback;
}

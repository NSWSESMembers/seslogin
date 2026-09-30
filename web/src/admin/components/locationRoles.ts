export type LocationRole = "admin" | "readOnly";

export interface LocationRoleEntry {
  locationId: string;
  role: LocationRole;
}

export interface LocationOption {
  id: string;
  name: string;
}

/** Splits entries into the two id lists the createUser/updateUser API takes. */
export function splitLocationRoles(entries: readonly LocationRoleEntry[]): {
  locationGrants: string[];
  readOnlyLocationGrants: string[];
} {
  return {
    locationGrants: entries
      .filter((e) => e.role === "admin")
      .map((e) => e.locationId),
    readOnlyLocationGrants: entries
      .filter((e) => e.role === "readOnly")
      .map((e) => e.locationId),
  };
}

/** Builds entries from a user's two grant lists (the API keeps them disjoint). */
export function joinLocationRoles(
  adminIds: readonly string[],
  readOnlyIds: readonly string[],
): LocationRoleEntry[] {
  return [
    ...adminIds.map((locationId) => ({ locationId, role: "admin" as const })),
    ...readOnlyIds.map((locationId) => ({
      locationId,
      role: "readOnly" as const,
    })),
  ];
}

import { formatFullDateTime } from "../../lib/time";

// Display helpers for the audit log page. The API sends enum values and raw
// stored field names; these turn them into the vocabulary the rest of the admin
// UI uses (a sign-in/out is "Activity", a kiosk session is a "Kiosk").

export const ENTITY_TYPE_LABELS = {
  USER: "User",
  PERSON: "Member",
  PERIOD: "Activity",
  SESSION: "Kiosk",
  API_TOKEN: "API token",
  LOCATION: "Location",
  CATEGORY: "Category",
  NITC_GROUP: "NITC group",
  NITC_TAG: "NITC tag",
  USER_TOKEN: "Login token",
  OAUTH_GRANT: "Connected app",
  WEBAUTHN_CREDENTIAL: "Passkey",
} as const;

export type EntityTypeKey = keyof typeof ENTITY_TYPE_LABELS;

export const ACTION_LABELS = {
  CREATE: "Created",
  UPDATE: "Updated",
  DELETE: "Deleted",
  RESTORE: "Restored",
} as const;

export function entityTypeLabel(entityType: string): string {
  return (
    (ENTITY_TYPE_LABELS as Record<string, string>)[entityType] ?? entityType
  );
}

export function actionLabel(action: string): string {
  return (ACTION_LABELS as Record<string, string>)[action] ?? action;
}

// The text for who did it. `label` is the looked-up name (email, kiosk name…);
// when it is missing (record deleted, nothing recorded) fall back to something
// that still says what kind of actor it was.
export function actorText(actor: {
  kind: string;
  id: string | null | undefined;
  label: string | null | undefined;
}): string {
  if (actor.label) return actor.label;
  switch (actor.kind) {
    case "USER":
      return "Unknown user";
    case "SESSION":
      return "Kiosk";
    case "API_TOKEN":
      return "API token";
    case "PERIOD_LINK":
      return "Period edit link";
    case "SYSTEM":
      return actor.id ? `System: ${actor.id}` : "System";
    case "UNAUTHENTICATED":
      return "Not signed in";
    default:
      return "Unknown";
  }
}

// A short description of the kind of actor, shown under the name when the name
// alone would not say (a kiosk called "Front desk", an API token).
export function actorKindHint(kind: string): string | null {
  switch (kind) {
    case "SESSION":
      return "Kiosk";
    case "API_TOKEN":
      return "API token";
    default:
      return null;
  }
}

const REDACTED = "[redacted]";

export function isRedacted(value: string | null | undefined): boolean {
  return value === REDACTED;
}

const TIME_FIELDS = new Set([
  "expires_at",
  "deleted",
  "missing_since",
  "start_time",
  "end_time",
]);

function isTimeField(field: string): boolean {
  return (
    TIME_FIELDS.has(field) || field.endsWith("_time") || field.endsWith("_at")
  );
}

// A field's before/after text as shown to the user: "—" when absent, a local
// date/time for epoch-seconds fields, otherwise the text as sent.
export function formatChangeValue(
  field: string,
  value: string | null | undefined,
): string {
  if (value == null) return "—";
  if (isTimeField(field) && /^\d+$/.test(value)) {
    const seconds = Number(value);
    if (seconds > 0 && Number.isSafeInteger(seconds)) {
      return formatFullDateTime(new Date(seconds * 1000));
    }
  }
  return value === "" ? "(empty)" : value;
}

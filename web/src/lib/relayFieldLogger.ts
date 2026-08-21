import type { RelayFieldLogger } from "relay-runtime";

type RelayFieldLoggerEvent = Parameters<RelayFieldLogger>[0];

/**
 * Describes a Relay field-level event for the console: the field path, its owning
 * query/fragment, and (when present) the underlying error message. These events fire
 * when a query or mutation response is partial — `data` populated alongside an
 * `errors` entry — which normalizes into the store silently unless something is
 * watching for it.
 */
function describe(event: RelayFieldLoggerEvent): string {
  switch (event.kind) {
    case "relay_field_payload.error":
      return `${event.owner} — field error at \`${event.fieldPath}\`: ${event.error.message}`;
    case "relay_resolver.error":
      return `${event.owner} — resolver error at \`${event.fieldPath}\`: ${event.error.message}`;
    case "missing_expected_data.log":
    case "missing_expected_data.throw":
      return `${event.owner} — missing expected data at \`${event.fieldPath}\``;
    case "missing_required_field.log":
    case "missing_required_field.throw":
      return `${event.owner} — missing @required field at \`${event.fieldPath}\``;
    default:
      return `unrecognized Relay field event: ${JSON.stringify(event)}`;
  }
}

const isDev = import.meta.env.MODE === "development";

/**
 * Shared Relay field logger, installed on every environment. This never changes
 * request/response behaviour — it only reports events Relay already computed, so a
 * silently-degraded query or mutation response shows up somewhere instead of just
 * reaching the component as an unexplained null.
 */
export const relayFieldLogger: RelayFieldLogger = (event) => {
  const message = `[relay-field-error] ${describe(event)}`;
  if (isDev) {
    console.error(message, event);
  } else {
    console.warn(message);
  }
};

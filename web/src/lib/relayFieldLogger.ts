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
 * Real server-reported messages seen very recently, most recent last. Populated
 * only by event kinds that carry an actual `error.message` — see
 * `takeRecentFieldErrorMessages` for why this exists and its (deliberately narrow)
 * lifetime.
 */
let recentFieldErrorMessages: string[] = [];
const MAX_BUFFERED_MESSAGES = 5;

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

  if (
    event.kind === "relay_field_payload.error" ||
    event.kind === "relay_resolver.error"
  ) {
    recentFieldErrorMessages.push(event.error.message);
    if (recentFieldErrorMessages.length > MAX_BUFFERED_MESSAGES) {
      recentFieldErrorMessages.shift();
    }
  }
};

/**
 * Returns and clears the real message(s) `relayFieldLogger` has captured since the
 * last call, most recent last.
 *
 * Relay's own thrown-error text for a field-level failure is a hardcoded, generic
 * string in every case that isn't a client-side Relay Resolver — even
 * `relay_field_payload.error`, fired for an actual server-reported GraphQL error on
 * a queried field, throws "Unexpected response payload - check server logs for
 * details." and never includes the real message (see relay-runtime's
 * `handlePotentialSnapshotErrors.js`). But `relayFieldLogger` receives the real
 * message moments earlier, synchronously, in the same read that produces the throw
 * — so a caller that just caught a generic Relay error (see the `Relay: ` prefix
 * convention those messages share) can call this immediately afterward to recover
 * what actually went wrong.
 *
 * This only works because of that synchronous adjacency: the buffer is cleared on
 * every read so a stale message can't attach itself to an unrelated later error,
 * but by the same token this must be called right after catching the error, not
 * from a delayed or async context.
 */
export function takeRecentFieldErrorMessages(): string[] {
  const messages = recentFieldErrorMessages;
  recentFieldErrorMessages = [];
  return messages;
}

/** Every hardcoded message relay-runtime's field-error throw path can produce. */
const GENERIC_RELAY_MESSAGE = /^Relay: /;

/**
 * Message to show for an error just caught by an error boundary. For a generic
 * Relay field-level throw (see `takeRecentFieldErrorMessages`), substitutes the
 * real message the field logger captured moments earlier, if any; otherwise
 * returns the error's own message unchanged.
 *
 * Call this as early as possible after catching — e.g. from an error boundary's
 * `onError`/`fallbackRender`, not after an await or any other async gap.
 */
export function describeCaughtError(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  if (!GENERIC_RELAY_MESSAGE.test(message)) return message;

  const recent = takeRecentFieldErrorMessages();
  return recent.length > 0 ? recent.join("; ") : message;
}

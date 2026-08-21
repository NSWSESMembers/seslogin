interface GraphQLResponseErrorLike {
  message?: string | null;
  extensions?: { code?: unknown } | null;
}

function responseErrors(err: unknown): ReadonlyArray<GraphQLResponseErrorLike> {
  if (err == null) return [];
  // Relay attaches the raw GraphQL response to network errors as `.source`.
  const source = (
    err as { source?: { errors?: ReadonlyArray<GraphQLResponseErrorLike> } }
  ).source;
  return source?.errors ?? [];
}

/**
 * Extracts the server-reported GraphQL error message(s) from an error thrown by a
 * Relay mutation's `onError` callback, or `null` if this isn't a GraphQL error.
 *
 * Our GraphQL network layer returns the raw `{ data, errors }` payload, so when
 * the server reports a GraphQL error Relay surfaces a network error that carries
 * the original response (with its `errors` array) on `.source`. A genuine network
 * failure (fetch rejected, non-200 response) has no `.source`, so `null` here
 * distinguishes "the server said no" from "we couldn't reach the server".
 */
export function getServerErrorMessage(err: unknown): string | null {
  const messages = responseErrors(err)
    .map((e) => e?.message)
    .filter((m): m is string => Boolean(m));
  return messages.length > 0 ? messages.join("; ") : null;
}

/**
 * Extracts the machine-readable `extensions.code` (e.g. `"UNAUTHENTICATED"`,
 * `"FORBIDDEN"`, `"NOT_FOUND"`) from the first GraphQL error that has one, or
 * `null` if there isn't one — either because this isn't a GraphQL error, or
 * because the server didn't classify it (older behaviour, or an uncaught
 * internal failure).
 */
export function getErrorCode(err: unknown): string | null {
  for (const error of responseErrors(err)) {
    const code = error?.extensions?.code;
    if (typeof code === "string") return code;
  }
  return null;
}

/** Friendlier copy for codes worth overriding the server's raw message for. */
const FRIENDLY_MESSAGE_BY_CODE: Record<string, string> = {
  UNAUTHENTICATED: "You need to sign in again to do this.",
  FORBIDDEN: "You don't have access to do this.",
};

/**
 * Extracts a human-readable message from an error thrown by a Relay mutation's
 * `onError` callback (or any rejected promise). Prefers friendlier copy for a
 * classified auth failure, then the server-provided GraphQL message, falling back
 * to the Error's own message.
 */
export function getErrorMessage(err: unknown): string {
  if (err == null) return "Unknown error";

  const code = getErrorCode(err);
  if (code != null && code in FRIENDLY_MESSAGE_BY_CODE) {
    return FRIENDLY_MESSAGE_BY_CODE[code];
  }

  const serverMessage = getServerErrorMessage(err);
  if (serverMessage != null) return serverMessage;

  if (err instanceof Error && err.message) return err.message;
  if (typeof err === "string") return err;
  try {
    return JSON.stringify(err);
  } catch {
    return String(err);
  }
}

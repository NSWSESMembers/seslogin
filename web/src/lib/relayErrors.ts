/** One entry of a GraphQL response's `errors` array. */
export interface GraphQLResponseError {
  message: string;
  path?: ReadonlyArray<string | number>;
  extensions?: Record<string, unknown>;
}

/**
 * Thrown by `fetchGraphQL` when a mutation response has `errors` even though
 * `data` is non-null — the mutation itself succeeded server-side (the write
 * happened), but a nested field on its result (e.g. `Period.person`) failed to
 * resolve. Distinct from an ordinary Relay network error so a caller can tell the
 * two apart: this one means "recorded, but couldn't show you the full result",
 * not "nothing happened".
 *
 * `.source.errors` matches the shape `getServerErrorMessage` already reads, so
 * existing `onError` handlers keep working without any change.
 */
export class MutationFieldError extends Error {
  readonly source: { errors: ReadonlyArray<GraphQLResponseError> };

  constructor(errors: ReadonlyArray<GraphQLResponseError>) {
    super(
      `Mutation reported field error(s): ${errors.map((e) => e.message).join("; ")}`,
    );
    this.name = "MutationFieldError";
    this.source = { errors };
  }
}

export function isMutationFieldError(err: unknown): err is MutationFieldError {
  return err instanceof MutationFieldError;
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
  if (err == null) return null;

  // Relay attaches the raw GraphQL response to network errors as `.source`.
  const source = (
    err as {
      source?: { errors?: ReadonlyArray<{ message?: string | null }> };
    }
  ).source;
  const gqlMessages = source?.errors
    ?.map((e) => e?.message)
    .filter((m): m is string => Boolean(m));
  if (gqlMessages && gqlMessages.length > 0) {
    return gqlMessages.join("; ");
  }
  return null;
}

/**
 * Extracts a human-readable message from an error thrown by a Relay mutation's
 * `onError` callback (or any rejected promise). Prefers the server-provided
 * GraphQL message, falling back to the Error's own message.
 */
export function getErrorMessage(err: unknown): string {
  if (err == null) return "Unknown error";

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

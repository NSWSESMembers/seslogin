import { getGraphQLEndpoint } from "./api";
import { clientHeaders } from "./clientInfo";
import {
  blockClientUpdates,
  clearBlockClientUpdates,
} from "./clientUpdateLeases";
import { type RequestParameters, type Variables } from "relay-runtime";
import { MutationFieldError } from "./relayErrors";

const FIELD_ERROR_EVENT = "graphql-field-error";

export interface GraphQLFieldErrorDetail {
  /** Relay operation name, shown in the toast for error reporting. */
  operationName: string;
  /** Number of entries in the response's `errors` array. */
  errorCount: number;
}

/**
 * Fired when a **query** response comes back with a populated `errors` array —
 * a partial response: `data` is (at least partly) present but one or more fields
 * failed to resolve. Mutations don't emit this; their field errors throw
 * `MutationFieldError` and are surfaced through the normal `onError` path.
 */
function emitFieldError(detail: GraphQLFieldErrorDetail): void {
  window.dispatchEvent(new CustomEvent(FIELD_ERROR_EVENT, { detail }));
}

/** Subscribe to query field errors; returns an unsubscribe function. */
export function onGraphQLFieldError(
  handler: (detail: GraphQLFieldErrorDetail) => void,
): () => void {
  const listener = (e: Event) =>
    handler((e as CustomEvent<GraphQLFieldErrorDetail>).detail);
  window.addEventListener(FIELD_ERROR_EVENT, listener);
  return () => window.removeEventListener(FIELD_ERROR_EVENT, listener);
}

let requestLeaseCounter = 0;

function nextRequestLeaseId(): string {
  if (
    typeof crypto !== "undefined" &&
    typeof crypto.randomUUID === "function"
  ) {
    return `graphql:${crypto.randomUUID()}`;
  }
  requestLeaseCounter += 1;
  return `graphql:${requestLeaseCounter}`;
}

/**
 * Supplies the `Authorization` header value for a request. Either:
 *  - a static full header value (e.g. `Bearer <token>`), or `null` for none, or
 *  - an async producer called with the exact serialized request body — used by the
 *    kiosk key flow, which signs the body hash. The same body string is then sent, so
 *    the signature always matches what the server hashes.
 */
export type AuthHeaderProvider =
  string | null | ((body: string) => Promise<string | null>);

/**
 * `serverTime` (Unix seconds) from the body of a 401 that refused a kiosk signature
 * only because its timestamp was outside the server's window (`code: CLOCK_SKEW`, see
 * `clock_skew_error` in api/src/auth.rs); null for any other body.
 */
export function clockSkewServerTime(body: unknown): number | null {
  const errors = (body as { errors?: unknown } | null)?.errors;
  if (!Array.isArray(errors)) {
    return null;
  }
  for (const error of errors) {
    const extensions = (
      error as { extensions?: Record<string, unknown> } | null
    )?.extensions;
    if (
      extensions?.code === "CLOCK_SKEW" &&
      typeof extensions.serverTime === "number"
    ) {
      return extensions.serverTime;
    }
  }
  return null;
}

/**
 * A signed kiosk request refused because the device's clock is wrong. Its key and
 * session are fine, so this is not an "unauthorized": the kiosk stays enrolled and asks
 * for the clock to be fixed (see kiosk/lib/clockSkew.ts).
 */
export class ClockSkewError extends Error {
  readonly serverTimeSec: number;

  constructor(serverTimeSec: number) {
    super("This device's clock is wrong");
    this.name = "ClockSkewError";
    this.serverTimeSec = serverTimeSec;
  }
}

export async function fetchGraphQL(
  authHeader: AuthHeaderProvider,
  request: RequestParameters,
  variables: Variables,
  onUnauthorized: () => void,
  /**
   * For signed kiosk requests: called, instead of `onUnauthorized`, when a 401 says the
   * device's clock is wrong; the request then rejects with a {@link ClockSkewError}.
   */
  onClockSkew?: (serverTimeSec: number) => void,
) {
  console.log(
    "fetchGraphQL for request ",
    request.name,
    " variables:",
    variables,
  );
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...clientHeaders(),
  };
  // Serialize the body once so the auth producer signs exactly what we send.
  const body = JSON.stringify({ query: request.text, variables });
  const authValue =
    typeof authHeader === "function" ? await authHeader(body) : authHeader;
  if (authValue) {
    headers["Authorization"] = authValue;
  }
  const endpoint = getGraphQLEndpoint();
  let resp: Response;
  const leaseId = nextRequestLeaseId();
  blockClientUpdates(
    leaseId,
    `GraphQL request in-flight: ${request.name ?? "unknown"}`,
  );
  try {
    resp = await fetch(endpoint, {
      method: "POST",
      headers,
      body,
      cache: "no-store",
    });
  } catch (error) {
    throw new Error(
      `Failed to fetch GraphQL endpoint ${endpoint}: ${error instanceof Error ? error.message : String(error)}`,
      { cause: error },
    );
  } finally {
    clearBlockClientUpdates(leaseId);
  }
  if (resp.status === 401) {
    if (onClockSkew) {
      const serverTime = clockSkewServerTime(
        await resp.json().catch(() => null),
      );
      if (serverTime !== null) {
        onClockSkew(serverTime);
        throw new ClockSkewError(serverTime);
      }
    }
    onUnauthorized();
    throw new Error("Unauthorized");
  }
  if (!resp.ok) {
    throw new Error("Response failed.");
  }
  const responseBody = await resp.json();

  if (Array.isArray(responseBody?.errors) && responseBody.errors.length > 0) {
    const errors: ReadonlyArray<{ message?: string; path?: unknown }> =
      responseBody.errors;
    if (errors.length === 1) {
      console.log(`GraphQL field error for ${request.name}:`, errors[0]);
    } else {
      console.log(
        `GraphQL field errors for ${request.name}: ${errors.length} errors`,
        errors.map((e) => ({ path: e?.path, message: e?.message })),
      );
    }

    // Queries return the partial response to Relay (the field errors don't
    // throw), so nothing else tells the user their view may be incomplete.
    // Mutations are handled by the MutationFieldError path below instead.
    if (request.operationKind !== "mutation") {
      emitFieldError({
        operationName: request.name ?? "unknown",
        errorCount: errors.length,
      });
    }
  }

  // A mutation that reports a field error is a failure, not a partial success:
  // the write already happened (data is non-null), but some nested field on the
  // result couldn't be read back. Routing this into the same MutationFieldError
  // → onError path as any other mutation failure means every existing onError
  // handler covers it with no changes, instead of the field silently landing in
  // the response as an unexplained null that reaches onCompleted.
  if (
    request.operationKind === "mutation" &&
    Array.isArray(responseBody?.errors) &&
    responseBody.errors.length > 0
  ) {
    throw new MutationFieldError(responseBody.errors);
  }

  return responseBody;
}

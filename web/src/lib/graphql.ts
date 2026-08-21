import { getGraphQLEndpoint } from "./api";
import {
  CLIENT_VERSION_HEADER,
  getCurrentClientVersion,
} from "./clientVersion";
import {
  blockClientUpdates,
  clearBlockClientUpdates,
} from "./clientUpdateLeases";
import { type RequestParameters, type Variables } from "relay-runtime";
import { MutationFieldError } from "./relayErrors";
import { recordServerErrorMessages } from "./relayFieldLogger";

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
 *  - a static full header value (e.g. `Bearer <jwt>`), or `null` for none, or
 *  - an async producer called with the exact serialized request body — used by the
 *    kiosk key flow, which signs the body hash. The same body string is then sent, so
 *    the signature always matches what the server hashes.
 */
export type AuthHeaderProvider =
  string | null | ((body: string) => Promise<string | null>);

export async function fetchGraphQL(
  authHeader: AuthHeaderProvider,
  request: RequestParameters,
  variables: Variables,
  onUnauthorized: () => void,
) {
  console.log(
    "fetchGraphQL for request ",
    request.name,
    " variables:",
    variables,
  );
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    [CLIENT_VERSION_HEADER]: getCurrentClientVersion(),
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
    onUnauthorized();
    throw new Error("Unauthorized");
  }
  if (!resp.ok) {
    throw new Error("Response failed.");
  }
  const responseBody = await resp.json();

  // Capture the raw errors immediately, for every operation kind, regardless of
  // what Relay itself later makes of the response. This is the only reliable
  // place to get the real message: async-graphql omits a failed field's key from
  // the response entirely rather than sending it as explicit `null`, so Relay's
  // normalizer — which only attaches an error to the store when it sees an
  // explicit `null` — never gets the chance to, and the field-level events that
  // do carry a message essentially never fire. Logging here also means the
  // message shows up even for a query that silently degrades with no throw at
  // all (no @throwOnFieldError on that query, or the field absorbed the error).
  if (Array.isArray(responseBody?.errors) && responseBody.errors.length > 0) {
    const messages = (
      responseBody.errors as ReadonlyArray<{ message?: unknown }>
    )
      .map((e) => e?.message)
      .filter((m): m is string => typeof m === "string");
    if (messages.length > 0) {
      console.error(
        `[graphql-error] ${request.name ?? "unknown"}:`,
        responseBody.errors,
      );
      recordServerErrorMessages(messages);
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

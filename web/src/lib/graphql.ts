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
  return await resp.json();
}

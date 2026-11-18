import { getGraphQLEndpoint } from "../lib/api";
import { clientHeaders } from "../lib/clientInfo";

/**
 * Plain fetches rather than Relay, as on Tally's sign-in page: these pages carry
 * their credential (the `slms_` token) as an argument, not a header, and hold no
 * state worth a store.
 */
export interface GraphQLReply<T> {
  data?: T | null;
  errors?: { message: string; extensions?: { code?: string } }[];
}

export async function memberGraphQL<T>(
  query: string,
  variables: Record<string, unknown>,
): Promise<GraphQLReply<T>> {
  const resp = await fetch(getGraphQLEndpoint(), {
    method: "POST",
    headers: { "Content-Type": "application/json", ...clientHeaders() },
    body: JSON.stringify({ query, variables }),
    cache: "no-store",
  });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
  return (await resp.json()) as GraphQLReply<T>;
}

/** Whether the reply says the token no longer works. */
export function isUnauthenticated(reply: GraphQLReply<unknown>): boolean {
  return reply.errors?.[0]?.extensions?.code === "UNAUTHENTICATED";
}

/**
 * The URL fragment's parameters. The token and email go in the fragment, never
 * the query string, because browsers never send a fragment to the server.
 */
export function readFragment(hash: string): URLSearchParams {
  return new URLSearchParams(hash.replace(/^#/, ""));
}

export function settingsPath(token: string, email: string): string {
  return `/member/settings#${new URLSearchParams({ token, email }).toString()}`;
}

export function signInPath(email: string, expired: boolean): string {
  const params = new URLSearchParams();
  if (email) params.set("email", email);
  if (expired) params.set("expired", "1");
  const fragment = params.toString();
  return fragment ? `/member/sign-in#${fragment}` : "/member/sign-in";
}

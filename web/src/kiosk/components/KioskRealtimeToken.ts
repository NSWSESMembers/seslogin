import { fetchQuery, graphql, type IEnvironment } from "relay-runtime";
import type { KioskRealtimeTokenQuery } from "./__generated__/KioskRealtimeTokenQuery.graphql";

// These fields aren't read in this file: `fetchRealtimeToken` hands the whole
// payload back to KioskRealtimeProvider, which passes `tokenRequest` straight
// through to ably-js's authCallback and the channel names to its consumers. Same
// pattern (and same disable) as KioskTokenSessionFetcher.tsx and
// UserInfoProvider.tsx.
/* eslint-disable relay/unused-fields */
const query = graphql`
  query KioskRealtimeTokenQuery {
    kioskRealtimeToken {
      channel
      sessionChannel
      tokenRequest {
        keyName
        ttl
        capability
        clientId
        timestamp
        nonce
        mac
      }
    }
  }
`;
/* eslint-enable relay/unused-fields */

export type KioskRealtimeTokenPayload = NonNullable<
  KioskRealtimeTokenQuery["response"]["kioskRealtimeToken"]
>;

/** `null` means realtime is disabled server-side (no `ABLY_API_KEY`) — not an
 * error, just a signal to fall back to polling. */
export async function fetchRealtimeToken(
  environment: IEnvironment,
): Promise<KioskRealtimeTokenPayload | null> {
  const response = await fetchQuery<KioskRealtimeTokenQuery>(
    environment,
    query,
    {},
    { fetchPolicy: "network-only" },
  ).toPromise();
  return response?.kioskRealtimeToken ?? null;
}

import { useLazyLoadQuery } from "react-relay";
import type {
  CacheConfig,
  FetchPolicy,
  GraphQLTaggedNode,
  OperationType,
  RenderPolicy,
  VariablesOf,
} from "relay-runtime";
import { useRelayRetryFetchKey } from "./relayRetryContext";

interface RetryableLazyLoadQueryOptions {
  /**
   * An unrelated reason to force a refetch with the same variables (e.g. a
   * periodic auto-refresh counter). Composed with the retry fetchKey below
   * rather than overriding it, so either one alone busts the cache.
   */
  fetchKey?: string | number;
  fetchPolicy?: FetchPolicy;
  networkCacheConfig?: CacheConfig;
  UNSTABLE_renderPolicy?: RenderPolicy;
}

/**
 * `useLazyLoadQuery`, wired so "Try again" on the nearest RelayErrorBoundary
 * (with `canRetry`) actually refetches instead of silently rethrowing the
 * same cached error — see RelayErrorBoundary's doc comment for why plain
 * `useLazyLoadQuery` doesn't do that on its own. Use this in place of
 * `useLazyLoadQuery` for any query reachable from a `canRetry` boundary.
 */
export function useRetryableLazyLoadQuery<TQuery extends OperationType>(
  query: GraphQLTaggedNode,
  variables: VariablesOf<TQuery>,
  options?: RetryableLazyLoadQueryOptions,
): TQuery["response"] {
  const retryFetchKey = useRelayRetryFetchKey();
  const { fetchKey, ...rest } = options ?? {};

  return useLazyLoadQuery<TQuery>(query, variables, {
    ...rest,
    fetchKey:
      fetchKey === undefined ? retryFetchKey : `${fetchKey}-${retryFetchKey}`,
  });
}

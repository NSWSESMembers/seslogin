import { useSyncExternalStore } from "react";

/**
 * Build/deployment facts about the API server we're talking to, mirroring the
 * `environment` GraphQL query.
 */
export interface EnvironmentInfo {
  gitRev: string;
  isProdDb: boolean;
}

// A module-level store rather than a React context: the three areas that can
// learn this (admin's Suspense query, the kiosk's polling fetcher, home's one-off
// unauthenticated probe) have no common provider, and the two components that
// display it — TopBar and TitleBarShell — are each shared across two of them.
let current: EnvironmentInfo | null = null;
const listeners = new Set<() => void>();

/**
 * Record what the server told us. Safe to call repeatedly with the same values:
 * the kiosk fetcher polls every couple of minutes, and an unconditional write
 * would hand `useSyncExternalStore` a fresh object every time and re-render.
 */
export function setEnvironmentInfo(next: EnvironmentInfo) {
  if (
    current !== null &&
    current.gitRev === next.gitRev &&
    current.isProdDb === next.isProdDb
  ) {
    return;
  }

  current = next;
  for (const listener of listeners) {
    listener();
  }
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getSnapshot() {
  return current;
}

/** The server's environment info, or null until a query has reported it. */
export function useEnvironmentInfo(): EnvironmentInfo | null {
  return useSyncExternalStore(subscribe, getSnapshot);
}

/**
 * Whether the API is known to be running against a non-production database.
 *
 * "Not yet known" deliberately reads as false so the chrome never flashes a
 * warning colour before the answer arrives and then corrects itself.
 */
export function useNonProdDb(): boolean {
  const info = useEnvironmentInfo();
  return info !== null && !info.isProdDb;
}

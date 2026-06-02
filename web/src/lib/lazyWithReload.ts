import { lazy, type ComponentType } from "react";

const RELOAD_FLAG_PREFIX = "lazy-reload:";

/**
 * Wraps React.lazy so that a failed dynamic import (typically a stale chunk
 * 404 after a deploy) triggers a one-time full reload to fetch the current
 * deploy, rather than rendering a dead error screen. A sessionStorage flag
 * keyed by chunk name guards against reload loops if the chunk genuinely
 * cannot be loaded.
 */
export function lazyWithReload<T extends ComponentType<unknown>>(
  name: string,
  factory: () => Promise<{ default: T }>,
) {
  const flagKey = `${RELOAD_FLAG_PREFIX}${name}`;
  return lazy(async () => {
    try {
      const mod = await factory();
      // Loaded successfully; clear any prior reload marker.
      sessionStorage.removeItem(flagKey);
      return mod;
    } catch (error) {
      const alreadyReloaded = sessionStorage.getItem(flagKey) != null;
      if (!alreadyReloaded) {
        sessionStorage.setItem(flagKey, "1");
        console.warn(`Failed to load chunk "${name}", reloading`, error);
        window.location.reload();
        // Return a never-resolving promise so we stay suspended until reload.
        return new Promise<{ default: T }>(() => {});
      }
      // Already reloaded once and still failing — surface the error.
      throw error;
    }
  });
}

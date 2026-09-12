import { useSyncExternalStore } from "react";

/**
 * Whether a CSS media query currently matches, kept in sync as the viewport
 * changes.
 *
 * `useSyncExternalStore` rather than `useState` + an effect so the first render
 * already has the right answer: a component that swaps which control it mounts
 * on the result would otherwise flash the wrong one.
 *
 * `matchMedia` is guarded the same way `lib/clientInfo.ts` guards it — jsdom
 * does not implement it, so tests get `fallback` unless they stub it (see
 * `setupTests.ts`, which does).
 */
export function useMediaQuery(query: string, fallback = true): boolean {
  function subscribe(onChange: () => void): () => void {
    if (typeof window.matchMedia !== "function") return () => {};
    const list = window.matchMedia(query);
    list.addEventListener("change", onChange);
    return () => list.removeEventListener("change", onChange);
  }

  function getSnapshot(): boolean {
    if (typeof window.matchMedia !== "function") return fallback;
    return window.matchMedia(query).matches;
  }

  return useSyncExternalStore(subscribe, getSnapshot, () => fallback);
}

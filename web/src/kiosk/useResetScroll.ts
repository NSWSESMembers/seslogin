import { useCallback, useEffect, useRef } from "react";

/**
 * Ref (and a manual `reset`) for a scan screen's outer (scrollable) div that snaps it back to the top
 * whenever `key` changes.
 *
 * Scan screens stay mounted across members so their slide transition survives,
 * which also means their scroll position would survive: a Categories screen
 * scrolled to the bottom for one member would greet the next one still scrolled
 * down. Key it on whatever identifies the content (usually the transaction uuid).
 */
export function useResetScroll<T extends HTMLElement>(key: unknown) {
  const ref = useRef<T>(null);
  const reset = useCallback(() => {
    if (ref.current) ref.current.scrollTop = 0;
  }, []);
  useEffect(reset, [reset, key]);
  return { ref, reset };
}

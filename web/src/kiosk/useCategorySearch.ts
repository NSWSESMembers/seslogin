import { useEffect, useRef, useState } from "react";
import { isSearchableQuery } from "./lib/categorySearch";

// What a barcode scanner types is a member ID: at least 8 digits.
const MEMBER_ID_DIGITS = 8;

/** `value` with any member IDs taken out (spaces inside one are allowed), or
 * null if it has none. One pass, so a huge paste can't be slow. */
function withoutScannedIds(value: string): string | null {
  let kept = "";
  let copied = 0;
  let digits = 0;
  let first = -1;
  let last = -1;
  let found = false;
  function endRun() {
    if (digits >= MEMBER_ID_DIGITS) {
      kept += value.slice(copied, first);
      copied = last + 1;
      found = true;
    }
    digits = 0;
    first = -1;
  }
  for (let i = 0; i < value.length; i++) {
    const char = value[i];
    if (char >= "0" && char <= "9") {
      if (first === -1) first = i;
      last = i;
      digits += 1;
    } else if (!/\s/.test(char)) {
      endRun();
    }
  }
  endRun();
  return found ? kept + value.slice(copied) : null;
}
export const SCANNED_HINT =
  "Finish or cancel this sign-out before scanning another member.";

/**
 * The text in a search box, kept apart from what it finds so the Quick pick
 * screen can share it. A barcode scanner types the member's ID, a digit at a
 * time, into whichever box has the cursor. Another member can't be signed in
 * while a sign-out screen is up, so the digits are taken out again, whatever was
 * already typed is kept, and `scanned` says why. `onEmptied` runs when the search
 * stops being long enough to search on.
 */
export function useSearchQuery(onEmptied?: () => void) {
  const [query, setQuery] = useState("");
  const [scanned, setScanned] = useState(false);

  function changeQuery(value: string) {
    const update = (next: string) => {
      setQuery(next);
      if (!isSearchableQuery(next)) onEmptied?.();
    };
    const stripped = withoutScannedIds(value);
    if (stripped !== null) {
      // A whole ID, or one tacked on to (or into) a search.
      setScanned(true);
      update(stripped);
    } else if (
      scanned &&
      value.startsWith(query) &&
      /^\d+$/.test(value.slice(query.length))
    ) {
      // The rest of an ID longer than 8 digits.
    } else {
      setScanned(false);
      update(value);
    }
  }

  return { query, scanned, changeQuery };
}

/** A ref for the search box, which gets the cursor when its screen comes on. The
 * scan screens stay mounted, so this waits until the screen is showing; without
 * `preventScroll` the browser would scroll the sliding screen to the input. */
export function useAutoFocusSearch(
  enabled: boolean | undefined,
  autoFocus: boolean | undefined,
  active: boolean | undefined,
) {
  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (enabled && autoFocus && active) {
      inputRef.current?.focus({ preventScroll: true });
    }
  }, [enabled, autoFocus, active]);
  return inputRef;
}

import { tw } from "../../lib/tw";

/**
 * Every colour here is an existing `@theme` token, so dark mode follows
 * automatically with no `dark:` variants — the same arrangement as `Dialog` and
 * `OptionList`.
 */

/** `pr-12` leaves room for the clear button and the chevron. */
export const comboboxInput = tw`h-7.5 pr-12 text-sm max-sm:h-9 max-sm:text-base`;

export const comboboxChevron = tw`pointer-events-none absolute top-1/2 right-2 -translate-y-1/2 text-xs text-ink-muted`;

export const comboboxClear = tw`absolute top-1/2 right-6 -translate-y-1/2 cursor-pointer rounded px-1 text-ink-muted transition-colors hover:text-ink focus:ring-2 focus:ring-menu/25 focus:outline-none`;

export const comboboxListbox = tw`z-50 m-0 list-none overflow-y-auto rounded-md border border-line bg-surface-raised p-1 shadow-lg`;

/**
 * Labels wrap rather than truncate — a 40-character category name does not fit
 * a phone width, and clipping it hides exactly the tail that distinguishes it
 * from its neighbours. `min-h-11` gives a 44px touch target on small screens.
 */
export const comboboxOption = tw`cursor-pointer rounded px-2 py-1 text-sm wrap-break-word text-ink data-active:bg-brand/10 max-sm:min-h-11 max-sm:py-2 max-sm:text-base`;

export const comboboxOptionDisabled = tw`cursor-default text-ink-muted opacity-60`;

export const comboboxOptionSelected = tw`font-semibold`;

/** The run of a label that the query matched, per `comboboxMatch.matchRanges`. */
export const comboboxHighlight = tw`font-semibold text-ink-strong`;

export const comboboxDescription = tw`block text-xs text-ink-muted`;

export const comboboxEmpty = tw`px-2 py-2 text-center text-sm text-ink-muted`;

export const comboboxWarning = tw`mt-1 text-sm text-red-600`;

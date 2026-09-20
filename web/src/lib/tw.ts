import { twMerge } from "tailwind-merge";

/** Identity helper so prettier-plugin-tailwindcss sorts these strings. */
export const tw = (strings: TemplateStringsArray, ...values: string[]) =>
  String.raw(strings, ...values);

/**
 * Merges Tailwind class strings, letting a later conflicting utility (e.g. a
 * caller-supplied `className`) win regardless of stylesheet order. Use this
 * instead of `[a, b].filter(Boolean).join(" ")` wherever a component merges
 * its own classes with a passed-in `className`.
 */
export const cn = (...classes: Array<string | false | null | undefined>) =>
  twMerge(classes.filter(Boolean).join(" "));

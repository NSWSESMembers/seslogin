/** Identity helper so prettier-plugin-tailwindcss sorts these strings. */
export const tw = (strings: TemplateStringsArray, ...values: string[]) =>
  String.raw(strings, ...values);

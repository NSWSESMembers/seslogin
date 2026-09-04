/**
 * Category icon markup, inlined at build time.
 *
 * These are ordinary `.svg` files (open fine in any design tool - see the color
 * comment in `app.css`) rather than JSX components: the `icon` string on each
 * category comes from data, so a plain `Record<icon, markup>` needs no
 * translation step. `import.meta.glob(..., {eager: true})` reads every file's
 * raw text at build time, so rendering is a DOM injection with no runtime fetch.
 */
const modules = import.meta.glob<string>("./*.svg", {
  query: "?raw",
  import: "default",
  eager: true,
});

// The source files are formatted with whitespace between tags for
// readability; inlined via innerHTML that whitespace becomes real text nodes
// (unlike an `<img>`, which has none), so strip it here rather than in every
// source file.
function stripInterTagWhitespace(svg: string): string {
  return svg.replace(/>\s+</g, "><").trim();
}

export const categoryIconMarkup: Record<string, string> = Object.fromEntries(
  Object.entries(modules).map(([path, svg]) => [
    path.replace(/^\.\//, "").replace(/\.svg$/, ""),
    stripInterTagWhitespace(svg),
  ]),
);

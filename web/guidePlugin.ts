/**
 * Builds the user guides into the app. Each folder under web/guide/ (admin/,
 * kiosk/) is a guide; every Markdown page in it is compiled
 * (src/guide/compile.ts) into the `virtual:guides` module that the /guide pages
 * import. Screenshots under web/guide/screenshots/ become
 * ordinary Vite asset imports, so they get hashed, cacheable URLs. A guide error —
 * a broken link, a missing screenshot, missing frontmatter — fails `vite build`,
 * and so CI. In dev, editing a guide file or screenshot reloads it.
 */
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import type { Plugin } from "vite";
import { compileGuides, IMAGE_PLACEHOLDER } from "./src/guide/compile.ts";

const MODULE_ID = "virtual:guides";
const RESOLVED_ID = `\0${MODULE_ID}`;

export default function guidePlugin(guideDir: string): Plugin {
  // The dev server tolerates a not-yet-captured screenshot; a build never does.
  let serving = false;
  const screenshotDir = join(guideDir, "screenshots");
  // Screenshots are captured at 2x, so each displays at half its pixel size.
  const images = () =>
    new Map(
      (existsSync(screenshotDir) ? readdirSync(screenshotDir) : [])
        .filter((f) => f.endsWith(".png"))
        .map((f) => {
          const { width, height } = pngSize(join(screenshotDir, f));
          return [
            `screenshots/${f}`,
            { width: Math.round(width / 2), height: Math.round(height / 2) },
          ];
        }),
    );

  // Badge artwork the guides may show inline, as badge ids: "first-steps" for
  // public/image/badges/by-id/first-steps.svg.
  const badgeDir = join(guideDir, "..", "public", "image", "badges", "by-id");
  const badgeIcons = () =>
    new Set(
      (existsSync(badgeDir) ? readdirSync(badgeDir) : [])
        .filter((f) => f.endsWith(".svg"))
        .map((f) => f.slice(0, -".svg".length)),
    );

  // Every Markdown page, as [guide, file] pairs: web/guide/admin/kiosks.md is
  // ["admin", "kiosks.md"]. Folders other than screenshots/ are guides.
  const pageFiles = () =>
    readdirSync(guideDir, { withFileTypes: true })
      .filter((d) => d.isDirectory() && d.name !== "screenshots")
      .flatMap((d) =>
        readdirSync(join(guideDir, d.name))
          .filter((file) => file.endsWith(".md"))
          .map((file) => [d.name, file] as const),
      );

  function moduleCode() {
    const guides = compileGuides(
      pageFiles().map(([guide, file]) => ({
        guide,
        file,
        source: readFileSync(join(guideDir, guide, file), "utf8"),
      })),
      images(),
      { allowMissingImages: serving, badgeIcons: badgeIcons() },
    );
    const pages = Object.values(guides).flat();
    // Each screenshot a page uses becomes an import; its placeholder in the
    // compiled HTML is swapped for the imported URL when the module loads.
    const used = [
      ...new Set(
        pages.flatMap((p) =>
          [...p.html.matchAll(IMAGE_PLACEHOLDER)].map((m) => m[1]),
        ),
      ),
    ];
    const imports = used
      .map(
        (path, i) =>
          `import img${i} from ${JSON.stringify(join(guideDir, path))};`,
      )
      .join("\n");
    const urls = `{${used.map((path, i) => `${JSON.stringify(path)}: img${i}`).join(", ")}}`;
    return `${imports}
const urls = ${urls};
const guides = ${JSON.stringify(guides)};
for (const page of Object.values(guides).flat()) {
  page.html = page.html.replace(${IMAGE_PLACEHOLDER}, (_, path) => urls[path]);
}
export default guides;
`;
  }

  return {
    name: "guides",
    configResolved(config) {
      serving = config.command === "serve";
    },
    resolveId(id) {
      return id === MODULE_ID ? RESOLVED_ID : undefined;
    },
    load(id) {
      if (id !== RESOLVED_ID) return undefined;
      // Watch files, never directories: Vite's dev server treats every watched
      // path as a dependency and fails to resolve a folder.
      for (const [guide, file] of pageFiles())
        this.addWatchFile(join(guideDir, guide, file));
      for (const image of images().keys())
        this.addWatchFile(join(guideDir, image));
      return moduleCode();
    },
    configureServer(server) {
      server.watcher.add(guideDir);
      const reload = (file: string) => {
        if (!file.startsWith(guideDir)) return;
        const mod = server.moduleGraph.getModuleById(RESOLVED_ID);
        if (mod) server.moduleGraph.invalidateModule(mod);
        server.ws.send({ type: "full-reload" });
      };
      server.watcher.on("change", reload);
      server.watcher.on("add", reload);
      server.watcher.on("unlink", reload);
    },
  };
}

/** Width and height from a PNG's header (bytes 16-23 of the IHDR chunk). */
function pngSize(file: string) {
  const header = readFileSync(file).subarray(0, 24);
  return { width: header.readUInt32BE(16), height: header.readUInt32BE(20) };
}

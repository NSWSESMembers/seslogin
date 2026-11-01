/**
 * Compiles the admin user guide (web/guide/*.md) into the data the guide pages
 * render. Runs at build time only — from the Vite plugin in web/guidePlugin.ts —
 * so `marked` never reaches the browser bundle. Never import this from app code;
 * import `virtual:admin-guide` instead.
 *
 * Problems that would show a reader a broken guide (a missing title, a link to a
 * page or heading that doesn't exist) are errors, and fail the build.
 */
import { Marked, Renderer, type Token, type Tokens } from "marked";
import type { GuidePage, GuideSection } from "./types";
import { GUIDE_BASE, GUIDE_IDS, RESERVED_SLUGS } from "./constants";

export interface GuideSource {
  /** The guide (folder under web/guide) the page belongs to, e.g. "admin". */
  guide: string;
  /** File name within the guide's folder, e.g. "kiosks.md". */
  file: string;
  source: string;
}

export class GuideError extends Error {}

const CALLOUTS: Record<string, string> = {
  NOTE: "Note",
  TIP: "Tip",
  IMPORTANT: "Important",
  WARNING: "Warning",
};

export function slugify(text: string): string {
  return text
    .toLowerCase()
    .replace(/&[a-z]+;/g, "")
    .replace(/[^a-z0-9\s-]/g, "")
    .trim()
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-");
}

/** `key: value` lines between `---` fences at the very top of the file. */
function frontmatter(file: string, source: string) {
  const m = source.match(/^---\n([\s\S]*?)\n---\n?/);
  if (!m)
    throw new GuideError(
      `${file}: missing frontmatter (title, summary, order)`,
    );
  const fields: Record<string, string> = {};
  for (const line of m[1].split("\n")) {
    if (!line.trim()) continue;
    const kv = line.match(/^([a-z]+):\s*(.*)$/);
    if (!kv) throw new GuideError(`${file}: bad frontmatter line "${line}"`);
    fields[kv[1]] = kv[2].replace(/^"(.*)"$/, "$1");
  }
  for (const key of ["title", "summary", "order"]) {
    if (!fields[key])
      throw new GuideError(`${file}: frontmatter needs "${key}"`);
  }
  const order = Number(fields.order);
  if (!Number.isFinite(order))
    throw new GuideError(`${file}: "order" must be a number`);
  return { fields, order, body: source.slice(m[0].length) };
}

/** Plain text of a token tree, for the search index. */
function plain(tokens: Token[] | undefined): string {
  return text(tokens).replace(/\s+/g, " ").trim();
}

// Block-level tokens are separated by a space; inline ones already carry their
// own spacing, so joining those with a space would put one before every comma.
const BLOCK = new Set([
  "paragraph",
  "heading",
  "list_item",
  "blockquote",
  "code",
  "space",
  "hr",
]);

function text(tokens: Token[] | undefined): string {
  if (!tokens) return "";
  return tokens
    .map((t) => {
      let s: string;
      if (t.type === "list")
        s = (t as Tokens.List).items.map((i) => text(i.tokens)).join(" ");
      else if (t.type === "table") {
        const tt = t as Tokens.Table;
        s = [...tt.header, ...tt.rows.flat()]
          .map((c) => text(c.tokens))
          .join(" ");
      } else if ("tokens" in t && t.tokens) s = text(t.tokens);
      else s = "text" in t ? String(t.text) : "";
      return BLOCK.has(t.type) || t.type === "list" || t.type === "table"
        ? ` ${s} `
        : s;
    })
    .join("");
}

const escapeHtml = (s: string) =>
  s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");

/**
 * Stands in for an image's URL in compiled HTML until the Vite plugin swaps in
 * the bundled asset's real (hashed) URL.
 */
export const imagePlaceholder = (path: string) => `__GUIDE_IMAGE:${path}__`;
export const IMAGE_PLACEHOLDER = /__GUIDE_IMAGE:([^_]+?)__/g;

// Pages sit in web/guide/<guide>/ and screenshots in web/guide/screenshots/, so
// a page refers to one as ../screenshots/<name>.png (which previews on GitHub too).
const IMAGE_PATH = /^\.\.\/(screenshots\/[a-z0-9-]+\.(png|jpg|webp))$/;
// A badge's own artwork, served by the app from web/public/image/badges/by-id/.
// Unlike a screenshot it may sit inline — in a table cell, say — beside its name.
const BADGE_ICON_PATH = /^\/image\/badges\/by-id\/([a-z0-9-]+)\.svg$/;

export function compileGuides(
  sources: GuideSource[],
  /**
   * Images present under web/guide/, e.g. "screenshots/kiosks.png", with their
   * display size in CSS pixels (screenshots are captured at 2x).
   */
  images?: Map<string, { width: number; height: number }>,
  {
    allowMissingImages = false,
    badgeIcons,
  }: {
    /**
     * Dev server only: show a placeholder for a screenshot that hasn't been
     * captured yet instead of failing, so a page can reference it first.
     */
    allowMissingImages?: boolean;
    /**
     * Badge ids with artwork in web/public/image/badges/by-id/, e.g.
     * "first-steps". When given, a page referring to any other badge fails.
     */
    badgeIcons?: Set<string>;
  } = {},
): Record<string, GuidePage[]> {
  const files = sources.filter(
    (s) =>
      s.file.endsWith(".md") &&
      s.file !== "README.md" &&
      !s.file.startsWith("_"),
  );

  // Pass 1: parse every page and collect its heading ids, so links can be checked.
  const parsed = files.map(({ guide, file: name, source }) => {
    // Errors name the file the way an author sees it: "admin/kiosks.md".
    const file = `${guide}/${name}`;
    if (!GUIDE_IDS.includes(guide))
      throw new GuideError(
        `${file}: "${guide}" isn't a guide — add it to GUIDES in src/guide/constants.ts`,
      );
    const slug = name.replace(/\.md$/, "");
    if (!/^[a-z0-9-]+$/.test(slug))
      throw new GuideError(
        `${file}: file names must be lower-case-with-dashes.md`,
      );
    if (RESERVED_SLUGS.includes(slug))
      throw new GuideError(
        `${file}: "${slug}" is reserved for a guide route — pick another name`,
      );
    const { fields, order, body } = frontmatter(file, source);
    const tokens = new Marked().lexer(body);
    const ids = new Set<string>();
    for (const t of tokens) {
      if (t.type !== "heading") continue;
      const h = t as Tokens.Heading;
      if (h.depth === 1)
        throw new GuideError(
          `${file}: don't use "# " headings — the title comes from frontmatter`,
        );
      const id = slugify(plain(h.tokens));
      if (ids.has(id))
        throw new GuideError(`${file}: two headings would both be #${id}`);
      ids.add(id);
    }
    return { guide, file, slug, fields, order, tokens, ids };
  });
  const byKey = new Map(parsed.map((p) => [`${p.guide}/${p.slug}`, p]));

  // Pass 2: render, rewriting and checking links as we go.
  const pages = parsed.map((p) => {
    const errors: string[] = [];
    const resolveHref = (href: string): string => {
      if (/^(https?:|mailto:)/.test(href)) return href;
      // A screen of the app itself, e.g. /admin/members or /kiosk.
      if (/^\/[a-z]/.test(href)) return href;
      const m = href.match(
        /^(?:\.\.\/([a-z0-9-]+)\/)?([a-z0-9-]+)\.md(?:#([a-z0-9-]+))?$|^#([a-z0-9-]+)$/,
      );
      if (!m) {
        errors.push(
          `link "${href}" — use page.md, page.md#heading, ../other-guide/page.md, #heading, /app/path or https://`,
        );
        return href;
      }
      const name = `${m[1] ?? p.guide}/${m[2]}`;
      const target = m[4] ? p : byKey.get(name);
      const anchor = m[4] ?? m[3];
      if (!target) errors.push(`link "${href}" — there is no ${name}.md`);
      else if (anchor && !target.ids.has(anchor))
        errors.push(
          `link "${href}" — ${target.file} has no heading #${anchor}`,
        );
      if (!target) return href;
      return `${GUIDE_BASE}/${target.guide}/${target.slug}${anchor ? `#${anchor}` : ""}`;
    };

    const md = new Marked({ gfm: true });
    md.use({
      renderer: {
        heading({ tokens, depth }) {
          const id = slugify(plain(tokens));
          return `<h${depth} id="${id}">${this.parser.parseInline(tokens)}</h${depth}>\n`;
        },
        link({ href, title, tokens }) {
          const resolved = resolveHref(href);
          const external = /^https?:/.test(resolved);
          const attrs = [
            `href="${escapeHtml(resolved)}"`,
            title ? `title="${escapeHtml(title)}"` : "",
            external ? 'target="_blank" rel="noopener noreferrer"' : "",
          ].filter(Boolean);
          return `<a ${attrs.join(" ")}>${this.parser.parseInline(tokens)}</a>`;
        },
        paragraph({ tokens }) {
          // An image on a line of its own becomes a captioned figure, unless it
          // is a badge icon, which stays an inline image.
          if (
            tokens.length === 1 &&
            tokens[0].type === "image" &&
            !BADGE_ICON_PATH.test((tokens[0] as Tokens.Image).href)
          ) {
            const { href, text, title } = tokens[0] as Tokens.Image;
            if (!text.trim())
              errors.push(
                `image "${href}" — needs alt text describing what it shows`,
              );
            // "screenshots/<name>.png": the image's path within web/guide/.
            const path = href.match(IMAGE_PATH)?.[1];
            if (!path)
              errors.push(
                `image "${href}" — use ../screenshots/<lower-case-name>.png (or .jpg/.webp)`,
              );
            else if (images && !images.has(path)) {
              if (!allowMissingImages)
                errors.push(
                  `image "${href}" — no such file in web/guide/screenshots/; capture it first`,
                );
              else
                return `<figure class="guide-figure"><p class="guide-missing-image">Screenshot not captured yet: ${escapeHtml(path)} — run make guide-screenshots</p></figure>\n`;
            }
            const size = path ? images?.get(path) : undefined;
            const dims = size
              ? ` width="${size.width}" height="${size.height}"`
              : "";
            const caption = title
              ? `<figcaption>${escapeHtml(title)}</figcaption>`
              : "";
            return `<figure class="guide-figure"><img src="${imagePlaceholder(path ?? href)}" alt="${escapeHtml(text)}"${dims} loading="lazy">${caption}</figure>\n`;
          }
          return `<p>${this.parser.parseInline(tokens)}</p>\n`;
        },
        image({ href, text }) {
          const badge = href.match(BADGE_ICON_PATH)?.[1];
          if (!badge) {
            errors.push(`image "${href}" — put images on a line of their own`);
            return "";
          }
          if (!text.trim())
            errors.push(
              `image "${href}" — needs alt text describing what it shows`,
            );
          if (badgeIcons && !badgeIcons.has(badge))
            errors.push(
              `image "${href}" — no such badge in web/public/image/badges/by-id/`,
            );
          return `<img class="guide-badge" src="${escapeHtml(href)}" alt="${escapeHtml(text)}" width="40" height="40" loading="lazy">`;
        },
        html({ text }) {
          // Raw HTML would bypass the guide's styling and escaping rules.
          errors.push(
            `raw HTML "${text.trim().slice(0, 40)}" — write Markdown instead`,
          );
          return "";
        },
        blockquote({ tokens }) {
          // GitHub-style callouts: "> [!TIP]" on the first line of a quote.
          const first = tokens[0];
          const lead =
            first?.type === "paragraph"
              ? (first as Tokens.Paragraph).tokens[0]
              : undefined;
          const m =
            lead?.type === "text"
              ? (lead as Tokens.Text).text.match(/^\[!([A-Z]+)\]\s*/)
              : null;
          if (m && lead && CALLOUTS[m[1]]) {
            const text = lead as Tokens.Text;
            text.text = text.text.slice(m[0].length);
            text.raw = text.raw.slice(m[0].length);
            delete text.tokens;
            const kind = m[1].toLowerCase();
            return `<aside class="guide-callout guide-callout-${kind}"><p class="guide-callout-title">${CALLOUTS[m[1]]}</p>${this.parser.parse(tokens)}</aside>\n`;
          }
          if (m)
            errors.push(
              `callout [!${m[1]}] — use one of ${Object.keys(CALLOUTS).join(", ")}`,
            );
          return `<blockquote>${this.parser.parse(tokens)}</blockquote>\n`;
        },
        table(token) {
          // Wrap so a wide table scrolls on a phone instead of widening the page.
          return `<div class="guide-table">${Renderer.prototype.table.call(this, token)}</div>`;
        },
      },
    });
    const html = md.parser(p.tokens) as string;
    if (errors.length)
      throw new GuideError(errors.map((e) => `${p.file}: ${e}`).join("\n"));

    // Search index: one entry for the page's intro, one per ## / ### section.
    const sections: GuideSection[] = [];
    let current: GuideSection = { id: "", heading: p.fields.title, text: "" };
    const flush = () => {
      current.text = current.text.replace(/\s+/g, " ").trim();
      if (current.text || current.id) sections.push(current);
    };
    for (const t of p.tokens) {
      if (t.type === "heading") {
        flush();
        const text = plain((t as Tokens.Heading).tokens);
        current = { id: slugify(text), heading: text, text: "" };
      } else {
        current.text += ` ${plain("tokens" in t ? t.tokens : [t])}`;
      }
    }
    flush();

    const page: GuidePage = {
      guide: p.guide,
      slug: p.slug,
      title: p.fields.title,
      summary: p.fields.summary,
      order: p.order,
      html,
      headings: p.tokens
        .filter(
          (t): t is Tokens.Heading =>
            t.type === "heading" && (t as Tokens.Heading).depth === 2,
        )
        .map((h) => ({
          id: slugify(plain(h.tokens)),
          text: plain(h.tokens),
        })),
      sections,
      ...(p.fields.video ? { video: p.fields.video } : {}),
    };
    return page;
  });

  const guides: Record<string, GuidePage[]> = {};
  for (const id of GUIDE_IDS) guides[id] = [];
  for (const page of pages) guides[page.guide].push(page);
  for (const list of Object.values(guides))
    list.sort((a, b) => a.order - b.order || a.title.localeCompare(b.title));
  return guides;
}

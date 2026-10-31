import { describe, it, expect } from "vitest";
import { compileGuides, GuideError, slugify } from "./compile";

const page = (
  file: string,
  body: string,
  front = "title: T\nsummary: S\norder: 1",
  guide = "admin",
) => ({
  guide,
  file,
  source: `---\n${front}\n---\n${body}`,
});

/** Most tests only need one guide: compile and return the admin guide's pages. */
const compileGuide = (...args: Parameters<typeof compileGuides>) =>
  compileGuides(...args).admin;

describe("compileGuides", () => {
  it("reads frontmatter and orders pages", () => {
    const pages = compileGuide([
      page("b.md", "Hi", "title: Bee\nsummary: Second\norder: 20"),
      page(
        "a.md",
        "Hi",
        "title: Ay\nsummary: First\norder: 10\nvideo: 01-intro",
      ),
    ]);
    expect(pages.map((p) => [p.slug, p.title, p.summary])).toEqual([
      ["a", "Ay", "First"],
      ["b", "Bee", "Second"],
    ]);
    expect(pages[0].video).toBe("01-intro");
    expect(pages[1]).not.toHaveProperty("video");
  });

  it("skips the README and underscore files", () => {
    const pages = compileGuide([
      page("a.md", "Hi"),
      { guide: "admin", file: "README.md", source: "# Not a page" },
      { guide: "admin", file: "_draft.md", source: "nope" },
    ]);
    expect(pages.map((p) => p.slug)).toEqual(["a"]);
  });

  it("gives headings ids and lists the ## ones", () => {
    const [p] = compileGuide([
      page("a.md", "## Who's here\n\ntext\n\n### Sub part\n"),
    ]);
    expect(p.html).toContain('<h2 id="whos-here">');
    expect(p.html).toContain('<h3 id="sub-part">');
    expect(p.headings).toEqual([{ id: "whos-here", text: "Who's here" }]);
  });

  it("rewrites links to other pages and their headings", () => {
    const [a] = compileGuide([
      page(
        "a.md",
        "See [b](b.md), [part](b.md#part), [here](#top) and [members](/admin/members).\n\n## Top\n",
      ),
      page("b.md", "## Part\n"),
    ]);
    expect(a.html).toContain('href="/guide/admin/b"');
    expect(a.html).toContain('href="/guide/admin/b#part"');
    expect(a.html).toContain('href="/guide/admin/a#top"');
    expect(a.html).toContain('href="/admin/members"');
  });

  it("links between guides", () => {
    const guides = compileGuides([
      page("a.md", "See [using a kiosk](../kiosk/b.md#part)."),
      page("b.md", "## Part\n", undefined, "kiosk"),
    ]);
    expect(guides.admin[0].html).toContain('href="/guide/kiosk/b#part"');
    expect(guides.kiosk[0].guide).toBe("kiosk");
    expect(() =>
      compileGuides([page("a.md", "[x](../kiosk/nope.md)")]),
    ).toThrow(/there is no kiosk\/nope\.md/);
  });

  it("keeps every guide in the result, even an empty one", () => {
    expect(Object.keys(compileGuides([]))).toEqual(["admin", "kiosk"]);
  });

  it("rejects a folder that isn't a guide", () => {
    expect(() =>
      compileGuides([page("a.md", "x", undefined, "drafts")]),
    ).toThrow(/"drafts" isn't a guide/);
  });

  it("names the guide in errors", () => {
    expect(() => compileGuide([page("a.md", "[x](nope.md)")])).toThrow(
      /^admin\/a\.md: link "nope\.md"/,
    );
  });

  it("opens outside links in a new tab", () => {
    const [p] = compileGuide([page("a.md", "[x](https://example.com)")]);
    expect(p.html).toContain('target="_blank" rel="noopener noreferrer"');
  });

  it.each([
    ["missing heading", "[x](#nope)", /has no heading #nope/],
    ["unsupported link", "[x](../thing)", /use page\.md/],
    ["link to a missing page", "[x](nope.md)", /there is no admin\/nope\.md/],
    ["image outside screenshots/", "![pic](a.png)", /use \.\.\/screenshots\//],
    ["image without alt text", "![](../screenshots/a.png)", /needs alt text/],
    [
      "image inside a sentence",
      "See ![pic](../screenshots/a.png) here",
      /line of their own/,
    ],
    ["raw html", "<div>hi</div>", /raw HTML/],
    ["h1", "# Title", /don't use "# " headings/],
    ["duplicate heading", "## Same\n\n## Same", /both be #same/],
    ["unknown callout", "> [!DANGER]\n> x", /use one of NOTE/],
  ])("rejects a %s", (_, body, message) => {
    expect(() => compileGuide([page("a.md", body)])).toThrow(message);
  });

  it("rejects missing frontmatter and reserved names", () => {
    expect(() =>
      compileGuide([{ guide: "admin", file: "a.md", source: "no front" }]),
    ).toThrow(GuideError);
    expect(() =>
      compileGuide([page("a.md", "x", "title: T\norder: 1")]),
    ).toThrow(/needs "summary"/);
    expect(() => compileGuide([page("print.md", "x")])).toThrow(/reserved/);
    expect(() => compileGuide([page("Bad Name.md", "x")])).toThrow(
      /lower-case/,
    );
  });

  it("renders callouts", () => {
    const [p] = compileGuide([page("a.md", "> [!TIP]\n> Bookmark **it**.\n")]);
    expect(p.html).toContain('<aside class="guide-callout guide-callout-tip">');
    expect(p.html).toContain('<p class="guide-callout-title">Tip</p>');
    expect(p.html).toContain("Bookmark <strong>it</strong>.");
    expect(p.html).not.toContain("[!TIP]");
  });

  it("turns a standalone screenshot into a captioned figure", () => {
    const [p] = compileGuide(
      [
        page(
          "a.md",
          '![The Kiosks page](../screenshots/kiosks.png "Your kiosks")\n',
        ),
      ],
      new Map([["screenshots/kiosks.png", { width: 400, height: 200 }]]),
    );
    expect(p.html).toBe(
      '<figure class="guide-figure"><img src="__GUIDE_IMAGE:screenshots/kiosks.png__" alt="The Kiosks page" width="400" height="200" loading="lazy"><figcaption>Your kiosks</figcaption></figure>\n',
    );
  });

  it("rejects a screenshot that hasn't been captured", () => {
    expect(() =>
      compileGuide(
        [page("a.md", "![Kiosks](../screenshots/kiosks.png)")],
        new Map(),
      ),
    ).toThrow(/no such file/);
  });

  it("shows a placeholder for a missing screenshot on the dev server", () => {
    const [p] = compileGuide(
      [page("a.md", "![Kiosks](../screenshots/kiosks.png)")],
      new Map(),
      { allowMissingImages: true },
    );
    expect(p.html).toContain(
      "Screenshot not captured yet: screenshots/kiosks.png",
    );
  });

  it("shows a badge icon inline, even in a table or on its own line", () => {
    const badge = "![First Steps badge](/image/badges/by-id/first-steps.svg)";
    const [p] = compileGuide(
      [page("a.md", `| | Badge |\n|---|---|\n| ${badge} | x |\n\n${badge}\n`)],
      undefined,
      { badgeIcons: new Set(["first-steps"]) },
    );
    const img =
      '<img class="guide-badge" src="/image/badges/by-id/first-steps.svg" alt="First Steps badge" width="40" height="40" loading="lazy">';
    expect(p.html).toContain(`<td>${img}</td>`);
    expect(p.html).toContain(`<p>${img}</p>`);
    expect(p.html).not.toContain("guide-figure");
  });

  it("rejects a badge icon with no artwork or no alt text", () => {
    const badgeIcons = new Set(["first-steps"]);
    expect(() =>
      compileGuide(
        [page("a.md", "See ![Nope](/image/badges/by-id/nope.svg).")],
        undefined,
        { badgeIcons },
      ),
    ).toThrow(/no such badge/);
    expect(() =>
      compileGuide(
        [page("a.md", "See ![](/image/badges/by-id/first-steps.svg).")],
        undefined,
        { badgeIcons },
      ),
    ).toThrow(/needs alt text/);
  });

  it("wraps tables so they can scroll", () => {
    const [p] = compileGuide([
      page("a.md", "| a | b |\n|---|---|\n| 1 | 2 |\n"),
    ]);
    expect(p.html).toMatch(/^<div class="guide-table"><table>/);
  });

  it("indexes the intro and each section as plain text", () => {
    const [p] = compileGuide([
      page(
        "a.md",
        "Intro **bold**.\n\n## One\n\n- item [link](#two)\n\n### Two\n\n| h |\n|---|\n| cell |\n",
      ),
    ]);
    expect(p.sections).toEqual([
      { id: "", heading: "T", text: "Intro bold." },
      { id: "one", heading: "One", text: "item link" },
      { id: "two", heading: "Two", text: "h cell" },
    ]);
  });
});

describe("slugify", () => {
  it("matches the documented rule", () => {
    expect(slugify("Who's here")).toBe("whos-here");
    expect(slugify("Admin and Read only")).toBe("admin-and-read-only");
    expect(slugify("1. Sign in")).toBe("1-sign-in");
  });
});

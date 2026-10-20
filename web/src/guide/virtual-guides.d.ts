// Provided by web/guidePlugin.ts: every web/guide/<guide>/*.md compiled at build
// time. The types live here (not in types.ts) because an ambient module can't
// import a relative path; types.ts re-exports them for the rest of the app.
declare module "virtual:guides" {
  /** One searchable chunk of a guide page: its intro, or one ## / ### section. */
  export interface GuideSection {
    /** Heading anchor on the page; "" for the intro before the first heading. */
    id: string;
    heading: string;
    /** Plain text, for matching and snippets. */
    text: string;
  }

  export interface GuidePage {
    /** The guide it belongs to: its folder, e.g. "admin". */
    guide: string;
    /** From the file name: web/guide/admin/kiosks.md -> /guide/admin/kiosks. */
    slug: string;
    title: string;
    summary: string;
    order: number;
    /** Rendered from the repository's own Markdown at build time. */
    html: string;
    /** The page's ## headings, for its "On this page" list. */
    headings: { id: string; text: string }[];
    sections: GuideSection[];
    /**
     * Name of the walkthrough video for this page. Recorded in frontmatter now, so
     * pages are ready for it, but not rendered until video hosting is decided.
     */
    video?: string;
  }

  /** Every guide's pages, in contents order, keyed by guide id. */
  const guides: Record<string, GuidePage[]>;
  export default guides;
}

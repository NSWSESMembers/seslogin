import type { GuidePage } from "./types";

export interface GuideHit {
  /** Which guide the page is in, e.g. "admin". */
  guide: string;
  slug: string;
  pageTitle: string;
  /** Heading anchor, or "" for the page's intro. */
  id: string;
  heading: string;
  /** A short excerpt around the first match, for display. */
  snippet: string;
  score: number;
}

const SNIPPET = 160;

/**
 * Lower-cased query words, trimmed to a rough stem so "expired" finds "expires"
 * and "kiosks" finds "kiosk". Matching is by substring, so a stem matches every
 * longer form of the word.
 */
export function searchTerms(query: string): string[] {
  return query
    .toLowerCase()
    .split(/[^a-z0-9']+/)
    .filter((t) => t.length > 1)
    .map(stem);
}

function stem(word: string): string {
  if (word.length <= 4) return word;
  return word.replace(/(ing|ed|es|s|e)$/, "");
}

/**
 * Every term must appear somewhere in the section (its heading, text or page
 * title); headings count for more than body text, so "kiosk settings" ranks the
 * section *called* that above one that merely mentions both words.
 */
export function searchGuide(
  pages: GuidePage[],
  query: string,
  limit = 20,
): GuideHit[] {
  const terms = searchTerms(query);
  if (!terms.length) return [];
  const hits: GuideHit[] = [];

  for (const page of pages) {
    const title = page.title.toLowerCase();
    for (const s of page.sections) {
      const heading = s.heading.toLowerCase();
      const text = s.text.toLowerCase();
      let score = 0;
      let ok = true;
      for (const t of terms) {
        const inHeading = heading.includes(t);
        // The intro's heading *is* the page title; don't count it twice.
        const inTitle = s.id !== "" && title.includes(t);
        const count = text.split(t).length - 1;
        if (!inHeading && !inTitle && !count) {
          ok = false;
          break;
        }
        score += (inHeading ? 6 : 0) + (inTitle ? 2 : 0) + Math.min(count, 5);
      }
      if (!ok) continue;
      hits.push({
        guide: page.guide,
        slug: page.slug,
        pageTitle: page.title,
        id: s.id,
        heading: s.heading,
        snippet: snippet(s.text, terms),
        score,
      });
    }
  }
  return hits.sort((a, b) => b.score - a.score).slice(0, limit);
}

function snippet(text: string, terms: string[]): string {
  const lower = text.toLowerCase();
  const at = Math.min(
    ...terms.map((t) => lower.indexOf(t)).filter((i) => i >= 0),
    text.length,
  );
  if (text.length <= SNIPPET) return text;
  let start = Math.max(0, at - 50);
  if (start > 0) start = text.indexOf(" ", start) + 1 || start;
  const end = Math.min(text.length, start + SNIPPET);
  return `${start > 0 ? "…" : ""}${text.slice(start, end).trim()}${end < text.length ? "…" : ""}`;
}

/** Split `text` into plain and matching runs, for highlighting without HTML. */
export function highlightRuns(
  text: string,
  terms: string[],
): { text: string; match: boolean }[] {
  if (!terms.length) return [{ text, match: false }];
  const alternatives = terms
    .map((t) => t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
    .join("|");
  // Highlight the whole word a stem matched ("expires" for "expir"), not just the stem.
  const re = new RegExp(`(?:${alternatives})[a-z0-9']*`, "gi");
  const runs: { text: string; match: boolean }[] = [];
  let at = 0;
  for (const m of text.matchAll(re)) {
    if (m.index > at)
      runs.push({ text: text.slice(at, m.index), match: false });
    runs.push({ text: m[0], match: true });
    at = m.index + m[0].length;
  }
  if (at < text.length) runs.push({ text: text.slice(at), match: false });
  return runs;
}

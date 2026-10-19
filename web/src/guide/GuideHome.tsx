import { Link, useSearchParams } from "react-router";
import guides from "virtual:guides";
import { GUIDES } from "./constants";
import { SearchField, SearchResults } from "./GuideSearch";
import { guideHref } from "./links";
import { searchGuide, searchTerms } from "./search";
import { useDocumentTitle } from "./useDocumentTitle";

const ALL_PAGES = Object.values(guides).flat();

/** /guide: choose a guide, or search them all. */
export default function GuideHome() {
  const [params, setParams] = useSearchParams();
  const query = params.get("q") ?? "";
  const terms = searchTerms(query);
  useDocumentTitle(query ? `Search: ${query}` : null);

  return (
    <div className="mx-auto max-w-4xl px-4 py-12 sm:py-16">
      <h1 className="font-title text-4xl font-bold text-navy sm:text-5xl">
        How can we help?
      </h1>
      <p className="mt-3 max-w-2xl text-lg text-ink-muted">
        Guides to running your unit on SES Login, and to signing in and out at
        your unit's kiosk.
      </p>

      <div className="mt-8">
        <SearchField
          label="Search the guides"
          value={query}
          onChange={(q) => setParams(q ? { q } : {}, { replace: true })}
        />
      </div>

      {query && terms.length > 0 ? (
        <SearchResults
          hits={searchGuide(ALL_PAGES, query)}
          terms={terms}
          showGuide
        />
      ) : (
        <ul className="mt-10 grid gap-5 sm:grid-cols-2">
          {GUIDES.map((g) => {
            const pages = guides[g.id] ?? [];
            return (
              <li key={g.id}>
                <Link
                  to={guideHref(g.id)}
                  className="group flex h-full flex-col rounded-2xl border border-line bg-surface p-6 no-underline shadow-sm transition hover:-translate-y-0.5 hover:border-navy hover:shadow-lg"
                >
                  <span className="self-start rounded-full bg-surface-raised px-2.5 py-0.5 text-xs font-semibold tracking-wide text-ink-muted uppercase">
                    {g.audience}
                  </span>
                  <span className="mt-3 font-title text-2xl font-bold text-navy">
                    {g.title}
                  </span>
                  <span className="mt-2 flex-1 text-ink">{g.description}</span>
                  <span className="mt-5 text-sm font-semibold text-accent-dark group-hover:underline dark:text-accent-light">
                    {pages.length} topics, starting with{" "}
                    {pages[0]?.title ?? "the basics"} →
                  </span>
                </Link>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}

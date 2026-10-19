import { Link, useParams, useSearchParams } from "react-router";
import guides from "virtual:guides";
import NotFound from "../components/NotFound";
import { GUIDES } from "./constants";
import { SearchField, SearchResults } from "./GuideSearch";
import { guideHref } from "./links";
import { searchGuide, searchTerms } from "./search";
import { useDocumentTitle } from "./useDocumentTitle";
import { Breadcrumb } from "./GuideBits";

/** /guide/<guide>: the guide's topics, and search within it. */
export default function GuideContents() {
  const { guide = "" } = useParams();
  const meta = GUIDES.find((g) => g.id === guide);
  const [params, setParams] = useSearchParams();
  const query = params.get("q") ?? "";
  const terms = searchTerms(query);
  useDocumentTitle(meta?.title ?? null);
  if (!meta) return <NotFound />;
  const pages = guides[guide] ?? [];

  return (
    <div className="mx-auto max-w-4xl px-4 py-10 sm:py-14">
      <Breadcrumb items={[{ to: "/guide", label: "Guides" }]} />

      <div className="mt-4 flex flex-wrap items-end justify-between gap-4">
        <div>
          <p className="text-sm font-semibold tracking-wide text-ink-muted uppercase">
            {meta.audience}
          </p>
          <h1 className="mt-1 font-title text-4xl font-bold text-navy">
            {meta.title}
          </h1>
          <p className="mt-2 max-w-2xl text-lg text-ink-muted">
            {meta.description}
          </p>
        </div>
        <Link
          to={`/guide/${guide}/print`}
          className="rounded-full border border-line px-4 py-2 text-sm font-semibold text-ink no-underline transition hover:border-navy hover:text-navy print:hidden"
        >
          Print or save as PDF
        </Link>
      </div>

      <div className="mt-8">
        <SearchField
          label={`Search the ${meta.title.toLowerCase()}`}
          value={query}
          onChange={(q) => setParams(q ? { q } : {}, { replace: true })}
        />
      </div>

      {query && terms.length > 0 ? (
        <SearchResults
          hits={searchGuide(pages, query)}
          terms={terms}
          showGuide={false}
        />
      ) : (
        <nav aria-label={`${meta.title} contents`} className="mt-8">
          <ol className="grid gap-4 sm:grid-cols-2">
            {pages.map((page, i) => (
              <li key={page.slug}>
                <Link
                  to={guideHref(guide, page.slug)}
                  className="group flex h-full gap-4 rounded-xl border border-line bg-surface p-5 no-underline transition hover:border-navy hover:shadow-md"
                >
                  <span className="flex size-8 shrink-0 items-center justify-center rounded-full bg-surface-raised text-sm font-bold text-navy group-hover:bg-navy group-hover:text-surface">
                    {i + 1}
                  </span>
                  <span>
                    <span className="block font-title text-lg font-bold text-navy">
                      {page.title}
                    </span>
                    <span className="mt-1 block text-sm text-ink">
                      {page.summary}
                    </span>
                  </span>
                </Link>
              </li>
            ))}
          </ol>
        </nav>
      )}
    </div>
  );
}

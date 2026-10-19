import { Link } from "react-router";
import { GUIDES } from "./constants";
import { guideHref } from "./links";
import { highlightRuns, type GuideHit } from "./search";

/** The big search box on the home and contents pages. */
export function SearchField({
  value,
  onChange,
  label,
}: {
  value: string;
  onChange: (value: string) => void;
  label: string;
}) {
  return (
    <form
      role="search"
      onSubmit={(e) => e.preventDefault()}
      className="relative print:hidden"
    >
      <label htmlFor="guide-search" className="sr-only">
        {label}
      </label>
      <svg
        viewBox="0 0 24 24"
        aria-hidden="true"
        className="pointer-events-none absolute top-1/2 left-4 size-5 -translate-y-1/2 text-ink-muted"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
      >
        <circle cx="11" cy="11" r="7" />
        <path d="M20 20l-3.5-3.5" />
      </svg>
      <input
        id="guide-search"
        type="search"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={label}
        className="w-full rounded-xl border border-line bg-surface py-3.5 pr-4 pl-12 text-lg text-ink shadow-sm outline-none focus:border-navy focus:ring-4 focus:ring-navy/15"
      />
    </form>
  );
}

function Highlight({ text, terms }: { text: string; terms: string[] }) {
  return (
    <>
      {highlightRuns(text, terms).map((run, i) =>
        run.match ? (
          <mark key={i} className="guide-mark">
            {run.text}
          </mark>
        ) : (
          run.text
        ),
      )}
    </>
  );
}

export function SearchResults({
  hits,
  terms,
  showGuide,
}: {
  hits: GuideHit[];
  terms: string[];
  /** Name the guide on each result (when searching more than one). */
  showGuide: boolean;
}) {
  return (
    <section aria-label="Search results" className="mt-6">
      <p className="mb-3 text-sm text-ink-muted" role="status">
        {hits.length === 0
          ? "Nothing in the guides matches that. Try fewer or different words."
          : `${hits.length} ${hits.length === 1 ? "result" : "results"}`}
      </p>
      <ul className="space-y-3">
        {hits.map((hit) => {
          const guide = GUIDES.find((g) => g.id === hit.guide);
          return (
            <li key={`${hit.guide}/${hit.slug}#${hit.id}`}>
              <Link
                to={guideHref(hit.guide, hit.slug, hit.id)}
                className="block rounded-xl border border-line bg-surface p-4 no-underline transition hover:border-navy hover:shadow-md"
              >
                <span className="block font-semibold text-navy">
                  <Highlight text={hit.heading} terms={terms} />
                </span>
                <span className="mt-0.5 block text-xs text-ink-muted">
                  {[
                    showGuide ? guide?.title : null,
                    hit.id ? hit.pageTitle : null,
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </span>
                <span className="mt-1.5 block text-sm text-ink">
                  <Highlight text={hit.snippet} terms={terms} />
                </span>
              </Link>
            </li>
          );
        })}
      </ul>
    </section>
  );
}

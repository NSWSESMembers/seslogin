import { useEffect } from "react";
import { Link, useLocation, useParams } from "react-router";
import guides, { type GuidePage } from "virtual:guides";
import NotFound from "../components/NotFound";
import { cn } from "../lib/tw";
import { GUIDES } from "./constants";
import { Breadcrumb, PrintButton } from "./GuideBits";
import GuideProse from "./GuideProse";
import { guideHref } from "./links";
import { useDocumentTitle } from "./useDocumentTitle";

/** /guide/<guide>/<page>: one topic, with the guide's other topics alongside. */
export default function GuideArticle() {
  const { guide = "", slug = "" } = useParams();
  const { hash } = useLocation();
  const meta = GUIDES.find((g) => g.id === guide);
  const pages = guides[guide] ?? [];
  const index = pages.findIndex((p) => p.slug === slug);
  const page = pages[index];
  useDocumentTitle(page && meta ? `${page.title} · ${meta.title}` : null);

  // A new topic starts at the top, unless the link was to one of its headings.
  useEffect(() => {
    if (!hash) window.scrollTo(0, 0);
  }, [guide, slug, hash]);

  if (!meta || !page) return <NotFound />;
  const prev = pages[index - 1];
  const next = pages[index + 1];

  return (
    <div className="mx-auto grid max-w-6xl gap-10 px-4 py-8 sm:py-10 lg:grid-cols-[13rem_minmax(0,1fr)] xl:grid-cols-[13rem_minmax(0,1fr)_12rem]">
      <aside className="max-lg:hidden print:hidden">
        <TopicList
          className="sticky top-24"
          label={`${meta.title} topics`}
          title={meta.title}
          pages={pages}
          current={slug}
        />
      </aside>

      <article className="max-w-3xl min-w-0">
        <Breadcrumb
          items={[
            { to: "/guide", label: "Guides" },
            { to: guideHref(guide), label: meta.title },
          ]}
          current={page.title}
        />

        <details className="mt-4 rounded-xl border border-line bg-surface-raised px-4 py-3 lg:hidden print:hidden">
          <summary className="cursor-pointer text-sm font-semibold text-ink-strong">
            Topics in the {meta.title.toLowerCase()}
          </summary>
          <TopicList
            className="mt-3"
            label={`${meta.title} topics`}
            pages={pages}
            current={slug}
          />
        </details>

        <header className="mt-6 mb-8 border-b border-line pb-6">
          <h1 className="font-title text-4xl font-bold text-navy sm:text-[2.6rem] sm:leading-tight">
            {page.title}
          </h1>
          <p className="mt-3 text-xl text-ink-muted">{page.summary}</p>
          <div className="mt-5">
            <PrintButton label="Print this page" />
          </div>
        </header>

        {page.headings.length > 2 && (
          <OnThisPage
            page={page}
            className="mb-8 rounded-xl border border-line bg-surface-raised px-4 py-3 xl:hidden"
          />
        )}

        <GuideProse html={page.html} />

        <nav
          aria-label="More of this guide"
          className="mt-14 grid gap-4 border-t border-line pt-6 sm:grid-cols-2 print:hidden"
        >
          {prev ? (
            <Link
              to={guideHref(guide, prev.slug)}
              className="rounded-xl border border-line p-4 no-underline transition hover:border-navy"
            >
              <span className="block text-xs text-ink-muted">Previous</span>
              <span className="font-semibold text-navy">← {prev.title}</span>
            </Link>
          ) : (
            <span />
          )}
          {next && (
            <Link
              to={guideHref(guide, next.slug)}
              className="rounded-xl border border-line p-4 text-right no-underline transition hover:border-navy"
            >
              <span className="block text-xs text-ink-muted">Next</span>
              <span className="font-semibold text-navy">{next.title} →</span>
            </Link>
          )}
        </nav>
      </article>

      {page.headings.length > 0 && (
        <aside className="max-xl:hidden print:hidden">
          <OnThisPage page={page} className="sticky top-24" />
        </aside>
      )}
    </div>
  );
}

function TopicList({
  pages,
  current,
  label,
  title,
  className,
}: {
  pages: GuidePage[];
  current: string;
  label: string;
  title?: string;
  className?: string;
}) {
  return (
    <nav aria-label={label} className={className}>
      {title && (
        <p className="mb-3 text-xs font-semibold tracking-wide text-ink-muted uppercase">
          {title}
        </p>
      )}
      <ol className="space-y-0.5 text-sm">
        {pages.map((p) => (
          <li key={p.slug}>
            <Link
              to={guideHref(p.guide, p.slug)}
              aria-current={p.slug === current ? "page" : undefined}
              className={cn(
                "block rounded-lg px-3 py-1.5 no-underline transition-colors",
                p.slug === current
                  ? "bg-navy font-semibold text-surface"
                  : "text-ink hover:bg-surface-raised",
              )}
            >
              {p.title}
            </Link>
          </li>
        ))}
      </ol>
    </nav>
  );
}

function OnThisPage({
  page,
  className,
}: {
  page: GuidePage;
  className?: string;
}) {
  return (
    <nav aria-label="On this page" className={className}>
      <p className="mb-2 text-xs font-semibold tracking-wide text-ink-muted uppercase">
        On this page
      </p>
      <ul className="space-y-1.5 text-sm">
        {page.headings.map((h) => (
          <li key={h.id}>
            <Link
              to={`#${h.id}`}
              className="text-ink no-underline hover:text-navy hover:underline"
            >
              {h.text}
            </Link>
          </li>
        ))}
      </ul>
    </nav>
  );
}

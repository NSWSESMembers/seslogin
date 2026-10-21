import { useParams } from "react-router";
import guides from "virtual:guides";
import NotFound from "../components/NotFound";
import { GUIDES } from "./constants";
import { Breadcrumb, PrintButton } from "./GuideBits";
import GuideProse from "./GuideProse";
import { guideHref } from "./links";
import { useDocumentTitle } from "./useDocumentTitle";

/** /guide/<guide>/print: the whole guide on one page, each topic on a new sheet. */
export default function GuidePrint() {
  const { guide = "" } = useParams();
  const meta = GUIDES.find((g) => g.id === guide);
  useDocumentTitle(meta ? `${meta.title} (printable)` : null);
  if (!meta) return <NotFound />;
  const pages = guides[guide] ?? [];

  return (
    <div className="mx-auto max-w-3xl px-4 py-10">
      <Breadcrumb
        items={[
          { to: "/guide", label: "Guides" },
          { to: guideHref(guide), label: meta.title },
        ]}
        current="Printable version"
      />
      <div className="mt-4 mb-10 flex flex-wrap items-center justify-between gap-3 rounded-xl border border-line bg-surface-raised p-4 print:hidden">
        <p className="text-sm">
          The whole {meta.title.toLowerCase()}, ready to print or save as a PDF.
          Each topic starts on a new page.
        </p>
        <PrintButton label="Print" />
      </div>

      <header className="mb-10">
        <p className="text-sm font-semibold tracking-wide text-ink-muted uppercase">
          SESLogin
        </p>
        <h1 className="font-title text-4xl font-bold text-navy">
          {meta.title}
        </h1>
        <h2 className="mt-8 mb-2 text-lg font-semibold">Contents</h2>
        <ol className="list-decimal pl-6">
          {pages.map((page) => (
            <li key={page.slug} className="py-0.5">
              <a
                href={`#print-${page.slug}`}
                className="text-accent-dark underline dark:text-accent-light print:text-black print:no-underline"
              >
                {page.title}
              </a>
              <span className="text-ink-muted"> — {page.summary}</span>
            </li>
          ))}
        </ol>
      </header>

      {pages.map((page) => (
        <section
          key={page.slug}
          id={`print-${page.slug}`}
          className="mb-14 guide-page-break"
        >
          <h1 className="mb-1 font-title text-3xl font-bold text-navy">
            {page.title}
          </h1>
          <p className="mb-6 text-lg text-ink-muted">{page.summary}</p>
          <GuideProse html={page.html} eager />
        </section>
      ))}
    </div>
  );
}

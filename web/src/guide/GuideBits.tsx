import { Fragment } from "react";
import { Link } from "react-router";

/** "Guides › Admin guide › …", ending at the current page (not a link). */
export function Breadcrumb({
  items,
  current,
}: {
  items: { to: string; label: string }[];
  current?: string;
}) {
  return (
    <nav
      aria-label="Breadcrumb"
      className="text-sm text-ink-muted print:hidden"
    >
      {items.map((item, i) => (
        <Fragment key={item.to}>
          {i > 0 && <span aria-hidden="true"> › </span>}
          <Link to={item.to} className="text-ink-muted underline">
            {item.label}
          </Link>
        </Fragment>
      ))}
      {current && (
        <>
          <span aria-hidden="true"> › </span>
          <span aria-current="page">{current}</span>
        </>
      )}
    </nav>
  );
}

export function PrintButton({ label }: { label: string }) {
  return (
    <button
      type="button"
      onClick={() => window.print()}
      className="inline-flex cursor-pointer items-center gap-2 rounded-full border border-line bg-surface px-4 py-2 text-sm font-semibold text-ink transition hover:border-navy hover:text-navy print:hidden"
    >
      <svg
        viewBox="0 0 24 24"
        aria-hidden="true"
        className="size-4"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <path d="M6 9V3h12v6M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2M6 14h12v7H6z" />
      </svg>
      {label}
    </button>
  );
}

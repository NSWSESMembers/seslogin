import { useState, type FormEvent, type ReactNode } from "react";
import { Link, NavLink, useNavigate } from "react-router";
import logo from "../assets/logo.svg";
import { cn } from "../lib/tw";
import { GUIDES } from "./constants";

/**
 * The frame around every guide page: a slim sticky header (logo, the two guides,
 * search) and a footer. Deliberately its own design rather than the admin
 * dashboard's chrome — the guide is public, read on phones as often as on desktops,
 * and should feel like documentation, not like the app.
 */
export default function GuideLayout({ children }: { children: ReactNode }) {
  return (
    <div className="flex min-h-dvh flex-col bg-surface text-ink">
      <a
        href="#guide-main"
        className="sr-only rounded-md bg-navy px-3 py-2 text-surface focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:z-50"
      >
        Skip to content
      </a>

      <header className="sticky top-0 z-30 border-b border-line bg-surface/90 backdrop-blur-sm print:hidden">
        <div className="mx-auto flex h-16 max-w-6xl items-center gap-3 px-4 sm:gap-6">
          <Link
            to="/guide"
            className="flex shrink-0 items-center gap-2.5 no-underline"
          >
            <img src={logo} alt="" className="size-8" />
            <span className="font-title text-lg font-bold text-navy">
              SES Login{" "}
              <span className="font-normal text-ink-muted max-sm:hidden">
                Guide
              </span>
            </span>
          </Link>

          <nav aria-label="Guides" className="flex gap-1">
            {GUIDES.map((g) => (
              <NavLink
                key={g.id}
                to={`/guide/${g.id}`}
                className={({ isActive }) =>
                  cn(
                    "rounded-full px-3 py-1.5 text-sm font-semibold no-underline transition-colors",
                    isActive
                      ? "bg-navy text-surface"
                      : "text-ink hover:bg-surface-raised",
                  )
                }
              >
                {g.title.replace(" guide", "")}
              </NavLink>
            ))}
          </nav>

          <HeaderSearch />
          <Link
            to="/guide"
            aria-label="Search the guides"
            className="ml-auto rounded-full p-2 text-ink hover:bg-surface-raised md:hidden"
          >
            <svg
              viewBox="0 0 24 24"
              aria-hidden="true"
              className="size-5"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
            >
              <circle cx="11" cy="11" r="7" />
              <path d="M20 20l-3.5-3.5" />
            </svg>
          </Link>
        </div>
      </header>

      <main id="guide-main" className="flex-1">
        {children}
      </main>

      <footer className="border-t border-line print:hidden">
        <div className="mx-auto flex max-w-6xl flex-wrap items-center justify-between gap-3 px-4 py-6 text-sm text-ink-muted">
          <span>SES Login · NSW SES Volunteers</span>
          <span className="flex gap-5">
            <Link to="/" className="text-ink-muted underline">
              Home
            </Link>
            <a
              href="mailto:support@seslogin.com"
              className="text-ink-muted underline"
            >
              support@seslogin.com
            </a>
          </span>
        </div>
      </footer>
    </div>
  );
}

/** Search from anywhere: results open on the guide home page. */
function HeaderSearch() {
  const navigate = useNavigate();
  const [query, setQuery] = useState("");

  function submit(e: FormEvent) {
    e.preventDefault();
    if (query.trim()) navigate(`/guide?q=${encodeURIComponent(query.trim())}`);
  }

  return (
    <form
      role="search"
      onSubmit={submit}
      className="ml-auto w-full max-w-64 max-md:hidden"
    >
      <label htmlFor="guide-header-search" className="sr-only">
        Search the guides
      </label>
      <input
        id="guide-header-search"
        type="search"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Search…"
        className="w-full rounded-full border border-line bg-surface-raised px-4 py-1.5 text-sm text-ink outline-none focus:border-navy focus:ring-2 focus:ring-navy/20"
      />
    </form>
  );
}

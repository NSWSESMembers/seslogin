import { useEffect, type MouseEvent } from "react";
import { useLocation, useNavigate } from "react-router";

/**
 * A compiled guide page. The HTML comes from the repository's own Markdown,
 * rendered at build time (compile.ts rejects raw HTML in it), never from user
 * input. Internal links are routed in-app instead of reloading the page.
 */
export default function GuideProse({
  html,
  eager = false,
}: {
  html: string;
  /** Load every image now: printing may not wait for lazy ones. */
  eager?: boolean;
}) {
  const navigate = useNavigate();
  const { hash } = useLocation();

  useEffect(() => {
    if (!hash) return;
    document
      .getElementById(decodeURIComponent(hash.slice(1)))
      ?.scrollIntoView();
  }, [hash, html]);

  function onClick(e: MouseEvent<HTMLDivElement>) {
    const a = (e.target as HTMLElement).closest("a");
    const href = a?.getAttribute("href");
    if (
      !a ||
      !href?.startsWith("/") ||
      a.target ||
      e.metaKey ||
      e.ctrlKey ||
      e.shiftKey
    )
      return;
    e.preventDefault();
    navigate(href);
  }

  return (
    // Only re-routes ordinary <a href> links, which stay keyboard-operable.
    <div
      className="guide-prose"
      onClick={onClick}
      dangerouslySetInnerHTML={{
        __html: eager
          ? html.replaceAll('loading="lazy"', 'loading="eager"')
          : html,
      }}
    />
  );
}

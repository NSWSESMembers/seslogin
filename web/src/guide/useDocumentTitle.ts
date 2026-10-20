import { useEffect } from "react";

/** Sets the tab title to "<title> · SES Login Guide" while the page is shown. */
export function useDocumentTitle(title: string | null) {
  useEffect(() => {
    const previous = document.title;
    document.title = title ? `${title} · SES Login Guide` : "SES Login Guide";
    return () => {
      document.title = previous;
    };
  }, [title]);
}

import { useEffect } from "react";

/** Sets the tab title to "<title> · SESLogin Guide" while the page is shown. */
export function useDocumentTitle(title: string | null) {
  useEffect(() => {
    const previous = document.title;
    document.title = title ? `${title} · SESLogin Guide` : "SESLogin Guide";
    return () => {
      document.title = previous;
    };
  }, [title]);
}

import { GUIDE_BASE } from "./constants";

/** URL of a guide's contents, one of its pages, or a heading on that page. */
export function guideHref(
  guide: string,
  slug?: string,
  anchor?: string,
): string {
  return `${GUIDE_BASE}/${guide}${slug ? `/${slug}` : ""}${anchor ? `#${anchor}` : ""}`;
}

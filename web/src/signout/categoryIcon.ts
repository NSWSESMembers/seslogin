import { categories as fixtureGroups } from "../lib/categories";

export interface CategoryOption {
  id: string;
  name: string;
  icon?: string;
}

export interface CategoryGroup {
  id: string;
  name: string;
  icon?: string;
  options: CategoryOption[];
}

const OTHER_GROUP_ID = "__other__";

/**
 * The kiosk's category picker is two levels (e.g. "Training" groups AIIMS, Fit
 * for Role, ...) but only the leaf items are real backend category ids — the
 * groups exist only in this local fixture (`lib/categories`), not the API.
 * Groups a flat list of backend categories the same way, matching each by id
 * against the fixture's subcategories, so the sign-out page can drill down
 * the same way the kiosk does. Backend categories with no fixture match (the
 * fixture can lag the real category list) are bucketed into a catch-all
 * "Other" group rather than silently dropped.
 */
export function groupCategories(
  categories: ReadonlyArray<{ id: string; name: string }>,
): CategoryGroup[] {
  const consumed = new Set<string>();
  const groups: CategoryGroup[] = [];

  for (const fixtureGroup of fixtureGroups) {
    const options: CategoryOption[] = [];
    for (const sub of fixtureGroup.subcategories) {
      const match = categories.find((c) => c.id === sub.id);
      if (match) {
        options.push({ id: match.id, name: match.name, icon: sub.icon });
        consumed.add(match.id);
      }
    }
    if (options.length > 0) {
      groups.push({
        id: fixtureGroup.id,
        name: fixtureGroup.name,
        icon: fixtureGroup.icon,
        options,
      });
    }
  }

  const leftover = categories.filter((c) => !consumed.has(c.id));
  if (leftover.length > 0) {
    groups.push({
      id: OTHER_GROUP_ID,
      name: "Other",
      options: leftover.map((c) => ({ id: c.id, name: c.name })),
    });
  }

  return groups;
}

export function categoryIconSrc(icon: string): string {
  return `/image/categories-cas/${icon}.png`;
}

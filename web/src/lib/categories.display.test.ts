import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { categories } from "./categories";

/**
 * The API stores a copy of the kiosk's category presentation (group, short name,
 * icon name) for the mobile app; `api/assets/category-display.json` is that copy and
 * `load-category-display` backfills it into the database. The kiosk still reads
 * `categories.ts`, so the two must not drift.
 */
describe("api/assets/category-display.json", () => {
  it("matches the leaves of the kiosk category tree", () => {
    // Tests run from `web/` (jsdom makes `import.meta.url` unusable here).
    const file = resolve(process.cwd(), "../api/assets/category-display.json");
    const actual: unknown = JSON.parse(readFileSync(file, "utf8"));

    const expected = categories
      .flatMap((group) =>
        group.subcategories.map((sub) => ({
          id: sub.id,
          groupName: group.name,
          shortName: sub.name,
          icon: sub.icon,
        })),
      )
      .sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));

    expect(
      actual,
      "api/assets/category-display.json is out of date with web/src/lib/categories.ts. " +
        "Update the JSON (one entry per subcategory, sorted by id: id, groupName, " +
        "shortName, icon), then run `load-category-display` to backfill the database.",
    ).toEqual(expected);
  });
});

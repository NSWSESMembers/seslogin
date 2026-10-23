import type { Ref } from "react";
import { categories } from "../../lib/categories";
import {
  CategoriesHeader,
  CategoryButton,
  CategorySearchBar,
} from "./ScanScreenCategories";

/**
 * Invisible, inert copy of a category screen's header and one button at scale
 * 1, which `useCategoryScale` measures instead of hard-coding the layout's
 * numbers: whatever the real classes add up to is what gets scaled. It sits
 * inside the same side padding as a scan screen (`px-2.5`) so the grid width it
 * reports is the width the real buttons get. Its labels are placeholder text so
 * nothing here duplicates a real screen's text for queries by text.
 *
 * With category search on it also draws the (empty) search box.
 */
export default function CategoryScaleProbe({
  ref,
  categorySearch = false,
}: {
  ref: Ref<HTMLDivElement>;
  categorySearch?: boolean;
}) {
  const leaf = categories[0].subcategories[0];
  return (
    <div
      ref={ref}
      aria-hidden="true"
      inert
      style={{ "--cat-scale": 1 } as React.CSSProperties}
      className="pointer-events-none invisible absolute inset-x-0 top-0 px-2.5 text-center"
    >
      <CategoriesHeader groupName="Probe" backLabel="Probe" />
      {categorySearch && (
        <CategorySearchBar query="" onQueryChange={() => {}} />
      )}
      <ul className="pl-0" data-probe="grid">
        <CategoryButton
          id={leaf.id}
          name="Probe"
          icon={leaf.icon}
          onSelect={() => {}}
        />
      </ul>
    </div>
  );
}

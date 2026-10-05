import { useEffect, useState } from "react";
import { categories } from "../../lib/categories";
import type { Category } from "../../lib/categories";
import { scanViewProps, type ScreenPosition } from "../../styles";
import { useResetScroll } from "../useResetScroll";
import { CategoryIcon } from "../../components/CategoryIcon";

// Every dimension below is `<px at scale 1> * --cat-scale`; the scale is chosen
// once per kiosk (see categoryScale.ts) and inherited from the scan area. The
// fallback of 1 is the original large size, for use outside the scan area.
export function CategoryButton(props: {
  id: string;
  name: string;
  icon: string;
  onSelect: () => void;
}) {
  const { name, icon, onSelect } = props;

  return (
    <li className="inline-block list-none align-bottom">
      <button
        onClick={onSelect}
        className="m-[calc(12px*var(--cat-scale,1))] box-content flex h-[calc(115px*var(--cat-scale,1))] w-[calc(150px*var(--cat-scale,1))] cursor-pointer flex-col content-start items-stretch rounded-lg border-2 border-line-strong bg-surface-raised p-[calc(10px*var(--cat-scale,1))] text-[calc(18px*var(--cat-scale,1))] leading-[calc(28px*var(--cat-scale,1))] wrap-break-word text-ink active:bg-menu"
      >
        <CategoryIcon
          icon={icon}
          className="mx-auto block size-[calc(70px*var(--cat-scale,1))]"
        />
        {name}
      </button>
    </li>
  );
}

/**
 * The title row above a category list. Shared with the scale probe, which
 * measures it: the drill-down variant (with the back button) is the taller one.
 */
export function CategoriesHeader(props: {
  groupName?: string;
  onBack?: () => void;
  /** Only the probe overrides this, so its text never matches a real screen's. */
  backLabel?: string;
}) {
  return (
    <div
      data-probe="header"
      className="mt-5 flex items-center justify-center gap-3.75 text-3xl"
    >
      {props.groupName !== undefined ? (
        <>
          <button
            className="inline-flex shrink-0 cursor-pointer items-center gap-1.5 rounded-lg border-2 border-line-strong bg-surface-raised px-3.5 py-1.5 align-middle text-ink active:bg-menu"
            onClick={props.onBack}
          >
            <span aria-hidden="true">&#8592;</span>{" "}
            {props.backLabel ?? "Categories"}
          </button>
          <span className="align-middle opacity-60" aria-hidden="true">
            &gt;
          </span>
          <span className="align-middle">{props.groupName}</span>
        </>
      ) : (
        <span className="align-middle">Categories</span>
      )}
    </div>
  );
}

export function Inner(props: {
  onSelectCategory: (uuid: string, categoryId: string) => void;
  uuid: string | null;
  /** Called on mount and whenever the list swaps; the host resets its scroll. */
  onListChange: () => void;
}) {
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);

  // Drilling into or out of a subcategory swaps the whole list, so start at the
  // top. Runs on mount too, which covers a new uuid (Inner is keyed on it).
  const { onListChange } = props;
  useEffect(() => {
    onListChange();
  }, [onListChange, selectedCategory]);

  const selectedCategoryData = selectedCategory
    ? categories.find((c: Category) => c.id === selectedCategory)
    : null;
  const shownCategories = selectedCategoryData
    ? selectedCategoryData.subcategories || []
    : categories;

  const sortedCategories = shownCategories.sort((a, b) =>
    a.name.localeCompare(b.name),
  );

  function back() {
    setSelectedCategory(null);
  }

  function select(id: string) {
    if (selectedCategory === null) {
      setSelectedCategory(id);
    } else {
      if (props.uuid === null) {
        throw new Error("UUID is null");
      }
      props.onSelectCategory(props.uuid, id);
    }
  }

  return (
    <>
      <CategoriesHeader groupName={selectedCategoryData?.name} onBack={back} />
      <ul className="pl-0">
        {sortedCategories.map((category) => (
          <CategoryButton
            key={category.id}
            id={category.id}
            name={category.name}
            icon={category.icon}
            onSelect={() => select(category.id)}
          />
        ))}
      </ul>
    </>
  );
}

// we expose this wrapper just so we can reset inner state on UUID change without
// causing the container <div> to remount and lose CSS transition state
export default function ScanScreenCategories(props: {
  onSelectCategory: (uuid: string, categoryId: string) => void;
  screenPosition: ScreenPosition;
  uuid: string | null;
}) {
  const { ref: scrollRef, reset: onListChange } =
    useResetScroll<HTMLDivElement>(props.uuid);
  return (
    <div ref={scrollRef} {...scanViewProps(props.screenPosition)}>
      <Inner
        onListChange={onListChange}
        onSelectCategory={props.onSelectCategory}
        key={props.uuid}
        uuid={props.uuid}
      />
    </div>
  );
}

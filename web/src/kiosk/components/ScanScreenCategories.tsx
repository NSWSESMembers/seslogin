import { useState } from "react";
import { categories as categoriesFixture } from "../../lib/categories";
import type { Category } from "../../lib/categories";
import { scanView, scanViewPosition, type ScreenPosition } from "../../styles";

function CategoryButton(props: {
  id: string;
  name: string;
  icon: string;
  onSelect: () => void;
  small?: boolean;
}) {
  const { name, icon, onSelect, small } = props;
  const iconSrc = `/image/categories-cas/${icon}.png`;

  return (
    <li className="inline-block list-none align-bottom">
      <button
        onClick={onSelect}
        className={
          small
            ? "m-2 box-content flex h-21 w-28.75 cursor-pointer flex-col content-start rounded-lg border-2 border-line-strong bg-surface-raised p-1.75 text-sm wrap-break-word text-ink active:bg-menu"
            : "m-3 box-content flex h-28.75 w-37.5 cursor-pointer flex-col content-start rounded-lg border-2 border-line-strong bg-surface-raised p-2.5 text-lg wrap-break-word text-ink active:bg-menu"
        }
      >
        <img
          src={iconSrc}
          className={`mx-auto block ${small ? "max-h-12 max-w-12" : ""}`}
        />
        {name}
      </button>
    </li>
  );
}

export function Inner(props: {
  onSelectCategory: (uuid: string, categoryId: string) => void;
  uuid: string | null;
  smallCategories?: boolean;
}) {
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);

  const selectedCategoryData = selectedCategory
    ? categoriesFixture.find((c: Category) => c.id === selectedCategory)
    : null;
  const categories = selectedCategoryData
    ? selectedCategoryData.subcategories || []
    : categoriesFixture;

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
      <div className="mt-5 flex items-center justify-center gap-3.75 text-[2em]">
        {selectedCategoryData ? (
          <>
            <button
              className="inline-flex shrink-0 cursor-pointer items-center gap-1.5 rounded-lg border-2 border-line-strong bg-surface-raised px-3.5 py-1.5 align-middle text-ink active:bg-menu"
              onClick={back}
            >
              <span aria-hidden="true">&#8592;</span> Categories
            </button>
            <span className="align-middle opacity-60" aria-hidden="true">
              &gt;
            </span>
            <span className="align-middle">{selectedCategoryData.name}</span>
          </>
        ) : (
          <span className="align-middle">Categories</span>
        )}
      </div>
      <ul className="pl-0">
        {categories.map((category) => (
          <CategoryButton
            key={category.id}
            id={category.id}
            name={category.name}
            icon={category.icon}
            onSelect={() => select(category.id)}
            small={props.smallCategories}
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
  smallCategories?: boolean;
}) {
  return (
    <div className={`${scanView} ${scanViewPosition[props.screenPosition]}`}>
      <Inner
        onSelectCategory={props.onSelectCategory}
        key={props.uuid}
        uuid={props.uuid}
        smallCategories={props.smallCategories}
      />
    </div>
  );
}

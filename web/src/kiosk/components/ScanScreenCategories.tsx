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
            ? "m-2 box-content flex h-21 w-28.75 cursor-pointer flex-col content-start rounded-lg border-2 border-neutral-500 bg-neutral-100 p-1.75 text-sm wrap-break-word text-neutral-800 active:bg-menu"
            : "m-3 box-content flex h-28.75 w-37.5 cursor-pointer flex-col content-start rounded-lg border-2 border-neutral-500 bg-neutral-100 p-2.5 text-lg wrap-break-word text-neutral-800 active:bg-menu"
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

  let categories;
  if (selectedCategory) {
    categories =
      categoriesFixture.find((c: Category) => c.id === selectedCategory)
        ?.subcategories || [];
  } else {
    categories = categoriesFixture;
  }

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
      <div className="mt-5 text-[2em]">
        <span className="align-middle">Categories</span>
        {selectedCategory && (
          <button
            className="ml-12.5 inline-block cursor-pointer rounded-lg border-2 border-neutral-500 bg-neutral-100 p-2.5 align-middle text-neutral-800 active:bg-menu"
            onClick={back}
          >
            Back
          </button>
        )}
      </div>
      <ul className="clear-both pl-0">
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

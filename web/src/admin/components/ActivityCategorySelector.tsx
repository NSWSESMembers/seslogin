import { graphql } from "relay-runtime";
import { useLazyLoadQuery } from "react-relay";
import type { ActivityCategorySelectorQuery } from "./__generated__/ActivityCategorySelectorQuery.graphql";
import { inputBase } from "../../components/ui/inputStyles";

interface ActivityCategorySelectorProps {
  value: ReadonlyArray<string>;
  onChange: (categoryIds: string[]) => void;
}

export default function ActivityCategorySelector({
  value,
  onChange,
}: ActivityCategorySelectorProps) {
  const data = useLazyLoadQuery<ActivityCategorySelectorQuery>(
    graphql`
      query ActivityCategorySelectorQuery {
        categories {
          id
          name
        }
      }
    `,
    {},
  );

  const categories = data.categories.toSorted((a, b) =>
    a.name.localeCompare(b.name),
  );

  function toggle(categoryId: string, checked: boolean) {
    if (checked) {
      onChange([...value, categoryId]);
    } else {
      onChange(value.filter((id) => id !== categoryId));
    }
  }

  return (
    <details className="inline-block text-left">
      <summary
        className={[inputBase, "h-7.5 cursor-pointer text-sm leading-6"]
          .filter(Boolean)
          .join(" ")}
      >
        Categories: {value.length === 0 ? "All" : `${value.length} selected`}
      </summary>
      <div className="mt-2 flex w-64 flex-col gap-1">
        <div className="flex gap-3 text-sm">
          <a
            href="#"
            onClick={(e) => {
              e.preventDefault();
              onChange(categories.map((category) => category.id));
            }}
          >
            Select all
          </a>
          <a
            href="#"
            onClick={(e) => {
              e.preventDefault();
              onChange([]);
            }}
          >
            Clear
          </a>
        </div>
        <div className="flex max-h-56 flex-col gap-1 overflow-y-auto rounded-md border border-line p-2 text-sm">
          {categories.map((category) => (
            <div key={category.id}>
              <input
                type="checkbox"
                id={`activity-category-${category.id}`}
                checked={value.includes(category.id)}
                onChange={(e) => toggle(category.id, e.target.checked)}
              />
              &nbsp;
              <label htmlFor={`activity-category-${category.id}`}>
                {category.name}
              </label>
            </div>
          ))}
        </div>
      </div>
    </details>
  );
}

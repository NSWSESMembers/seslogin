import { graphql } from "relay-runtime";
import { useLazyLoadQuery } from "react-relay";
import type { ActivityCategorySelectorQuery } from "./__generated__/ActivityCategorySelectorQuery.graphql";

interface ActivityCategorySelectorProps {
  value: string;
  onChange: (categoryId: string) => void;
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

  return (
    <label className="activity-category-selector">
      Category
      <select value={value} onChange={(e) => onChange(e.target.value)}>
        <option value="">All categories</option>
        {data.categories
          .toSorted((a, b) => a.name.localeCompare(b.name))
          .map((category) => (
            <option value={category.id} key={category.id}>
              {category.name}
            </option>
          ))}
      </select>
    </label>
  );
}

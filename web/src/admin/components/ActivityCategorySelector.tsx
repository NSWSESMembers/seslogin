import { useRef, useState } from "react";
import { graphql } from "relay-runtime";
import { useLazyLoadQuery } from "react-relay";
import type { ActivityCategorySelectorQuery } from "./__generated__/ActivityCategorySelectorQuery.graphql";
import { inputBase } from "../../components/ui/inputStyles";
import { Button } from "../../components/ui/Button";

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
      query ActivityCategorySelectorQuery @throwOnFieldError {
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

  const detailsRef = useRef<HTMLDetailsElement>(null);
  // Local, unapplied selection edited while the dropdown is open. Only
  // committed to `onChange` (and thus re-fetches the report) when the user
  // clicks Apply, so ticking several boxes doesn't trigger a query per click.
  const [pending, setPending] = useState<string[]>([...value]);

  function toggle(categoryId: string, checked: boolean) {
    setPending((prev) =>
      checked ? [...prev, categoryId] : prev.filter((id) => id !== categoryId),
    );
  }

  function handleToggle() {
    // Re-sync pending selection from the applied value whenever the dropdown
    // opens, so a close-without-applying discards any edits made last time.
    if (detailsRef.current?.open) {
      setPending([...value]);
    }
  }

  function apply() {
    onChange(pending);
    if (detailsRef.current) {
      detailsRef.current.open = false;
    }
  }

  return (
    <details
      ref={detailsRef}
      className="inline-block text-left"
      onToggle={handleToggle}
    >
      <summary
        className={[inputBase, "h-7.5 cursor-pointer text-sm leading-6"]
          .filter(Boolean)
          .join(" ")}
      >
        Categories: {value.length === 0 ? "All" : `${value.length} selected`}
      </summary>
      <div className="mt-2 flex w-max min-w-64 flex-col gap-1">
        <div className="flex gap-3 text-sm">
          <a
            href="#"
            onClick={(e) => {
              e.preventDefault();
              setPending(categories.map((category) => category.id));
            }}
          >
            Select all
          </a>
          <a
            href="#"
            onClick={(e) => {
              e.preventDefault();
              setPending([]);
            }}
          >
            Clear
          </a>
        </div>
        <div className="flex max-h-56 flex-col gap-1 overflow-y-auto rounded-md border border-line p-2 text-sm">
          {categories.map((category) => (
            <div key={category.id} className="whitespace-nowrap">
              <input
                type="checkbox"
                id={`activity-category-${category.id}`}
                checked={pending.includes(category.id)}
                onChange={(e) => toggle(category.id, e.target.checked)}
              />
              &nbsp;
              <label htmlFor={`activity-category-${category.id}`}>
                {category.name}
              </label>
            </div>
          ))}
        </div>
        <div className="flex justify-end">
          <Button size="row" onClick={apply}>
            Apply
          </Button>
        </div>
      </div>
    </details>
  );
}
